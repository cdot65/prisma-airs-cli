# Function: computeMetrics()

> **computeMetrics**(`results`): [`EfficacyMetrics`](../interfaces/EfficacyMetrics.md)

Defined in: [src/core/metrics.ts:11](https://github.com/cdot65/prisma-airs-cli/blob/main/src/core/metrics.ts#L11)

Classify test results into TP/TN/FP/FN and compute efficacy metrics.

## Parameters

### results

[`TestResult`](../interfaces/TestResult.md)[]

Scan results paired with their expected outcomes.

## Returns

[`EfficacyMetrics`](../interfaces/EfficacyMetrics.md)

Confusion matrix counts, rates (TPR, TNR, accuracy, coverage, F1),
         and regression failure count. Division-by-zero yields 0.
         Coverage = min(TPR, TNR) — the loop's stop condition target.
