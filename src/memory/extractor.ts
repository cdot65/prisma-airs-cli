import type { BaseChatModel } from '@langchain/core/language_models/chat_models';
import { nanoid } from 'nanoid';
import type { RunState } from '../core/types.js';
import { computeIterationDiff } from './diff.js';
import { extractLearningsPrompt } from './prompts/extract-learnings.js';
import { type LearningExtractionOutput, LearningExtractionOutputSchema } from './schemas.js';
import { type MemoryStore, normalizeCategory } from './store.js';
import type { Learning, TopicMemory } from './types.js';

export class LearningExtractor {
  constructor(
    private model: BaseChatModel,
    private store: MemoryStore,
  ) {}

  async extractAndSave(runState: RunState): Promise<{ learnings: Learning[] }> {
    if (runState.iterations.length < 1) {
      return { learnings: [] };
    }

    const structured = this.model.withStructuredOutput(LearningExtractionOutputSchema);
    const chain = extractLearningsPrompt.pipe(structured);

    const iterationHistory = this.formatIterationHistory(runState);

    const raw = (await chain.invoke({
      topicDescription: runState.userInput.topicDescription,
      intent: runState.userInput.intent,
      totalIterations: runState.iterations.length,
      bestIteration: runState.bestIteration,
      bestCoverage: `${(runState.bestCoverage * 100).toFixed(1)}%`,
      iterationHistory,
    })) as LearningExtractionOutput;

    const category = normalizeCategory(runState.userInput.topicDescription);
    const bestIter =
      runState.iterations[runState.bestIteration - 1] ??
      runState.iterations[runState.iterations.length - 1];

    // Build learnings with metadata
    const newLearnings: Learning[] = raw.learnings.map((l) => ({
      id: nanoid(),
      runId: runState.id,
      extractedAt: new Date().toISOString(),
      topicCategory: category,
      topicDescription: runState.userInput.topicDescription,
      insight: l.insight,
      strategy: l.strategy,
      outcome: l.outcome,
      changeType: l.changeType ?? 'initial',
      metrics: {
        coverage: bestIter.metrics.coverage,
        tpr: bestIter.metrics.truePositiveRate,
        tnr: bestIter.metrics.trueNegativeRate,
        accuracy: bestIter.metrics.accuracy,
        f1: bestIter.metrics.f1Score,
      },
      corroborations: 0,
      tags: l.tags,
    }));

    // Merge into existing memory
    const existing = (await this.store.load(category)) ?? {
      category,
      updatedAt: '',
      learnings: [],
      bestKnown: null,
      antiPatterns: [],
    };

    const mergedLearnings = this.mergeLearnings(existing.learnings, newLearnings);

    // Update bestKnown only if new run is better
    let { bestKnown } = existing;
    if (!bestKnown || runState.bestCoverage > bestKnown.metrics.coverage) {
      bestKnown = {
        runId: runState.id,
        iteration: bestIter.iteration,
        topic: bestIter.topic,
        metrics: bestIter.metrics,
      };
    }

    // Merge anti-patterns (deduplicate)
    const antiPatternSet = new Set([...existing.antiPatterns, ...raw.antiPatterns]);

    const memory: TopicMemory = {
      category,
      updatedAt: new Date().toISOString(),
      learnings: mergedLearnings,
      bestKnown,
      antiPatterns: [...antiPatternSet],
    };

    await this.store.save(memory);

    return { learnings: newLearnings };
  }

  private mergeLearnings(existing: Learning[], incoming: Learning[]): Learning[] {
    const merged = [...existing];

    for (const newL of incoming) {
      const match = merged.findIndex((e) => e.insight === newL.insight);
      if (match >= 0) {
        // Corroborate: increment count, keep existing metrics
        merged[match] = {
          ...merged[match],
          corroborations: merged[match].corroborations + 1,
          extractedAt: newL.extractedAt,
        };
      } else {
        merged.push(newL);
      }
    }

    return merged;
  }

  private formatIterationHistory(runState: RunState): string {
    const lines: string[] = [];

    for (let i = 0; i < runState.iterations.length; i++) {
      const iter = runState.iterations[i];
      lines.push(`--- Iteration ${iter.iteration} ---`);
      lines.push(`Description: "${iter.topic.description}"`);
      lines.push(`Example count: ${iter.topic.examples.length}`);
      lines.push(
        `Examples: ${iter.topic.examples.length > 0 ? iter.topic.examples.map((e) => `"${e}"`).join(', ') : 'None (description-only)'}`,
      );
      lines.push(
        `Coverage: ${(iter.metrics.coverage * 100).toFixed(1)}% | TPR: ${(iter.metrics.truePositiveRate * 100).toFixed(1)}% | TNR: ${(iter.metrics.trueNegativeRate * 100).toFixed(1)}%`,
      );
      lines.push(`Analysis: ${iter.analysis.summary}`);

      if (i > 0) {
        const diff = computeIterationDiff(runState.iterations[i - 1], iter);
        const changes: string[] = [];
        if (diff.descriptionChanged) changes.push('description changed');
        if (diff.examplesChanged) {
          const prevCount = runState.iterations[i - 1].topic.examples.length;
          const currCount = iter.topic.examples.length;
          changes.push(`example count: ${prevCount} → ${currCount}`);
          if (diff.examplesAdded.length)
            changes.push(`+examples: ${diff.examplesAdded.join(', ')}`);
          if (diff.examplesRemoved.length)
            changes.push(`-examples: ${diff.examplesRemoved.join(', ')}`);
        }
        lines.push(`Changes: ${changes.join('; ') || 'none'}`);
        lines.push(
          `Coverage delta: ${diff.metricDelta.coverage >= 0 ? '+' : ''}${(diff.metricDelta.coverage * 100).toFixed(1)}%`,
        );
      }

      lines.push('');
    }

    return lines.join('\n');
  }
}
