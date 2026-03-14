import type { IterationResult } from '../core/types.js';
import type { IterationDiff } from './types.js';

export function computeIterationDiff(from: IterationResult, to: IterationResult): IterationDiff {
  const fromExSet = new Set(from.topic.examples);
  const toExSet = new Set(to.topic.examples);

  const examplesAdded = to.topic.examples.filter((e) => !fromExSet.has(e));
  const examplesRemoved = from.topic.examples.filter((e) => !toExSet.has(e));

  return {
    fromIteration: from.iteration,
    toIteration: to.iteration,
    descriptionChanged: from.topic.description !== to.topic.description,
    examplesChanged: examplesAdded.length > 0 || examplesRemoved.length > 0,
    examplesAdded,
    examplesRemoved,
    descriptionBefore: from.topic.description,
    descriptionAfter: to.topic.description,
    metricDelta: {
      coverage: to.metrics.coverage - from.metrics.coverage,
      tpr: to.metrics.truePositiveRate - from.metrics.truePositiveRate,
      tnr: to.metrics.trueNegativeRate - from.metrics.trueNegativeRate,
      accuracy: to.metrics.accuracy - from.metrics.accuracy,
      f1: to.metrics.f1Score - from.metrics.f1Score,
    },
  };
}
