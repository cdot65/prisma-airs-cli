---
title: Metrics & Evaluation
---

# Metrics & Evaluation

Every iteration measures how well the guardrail performs, then uses that data to guide improvement. Here's what gets measured and why.

## The Metrics

| Metric | Formula | What it tells you |
|--------|---------|-------------------|
| **TPR** (sensitivity) | `TP / (TP + FN)` | How many violations are caught |
| **TNR** (specificity) | `TN / (TN + FP)` | How many safe prompts are correctly passed |
| **Coverage** | `min(TPR, TNR)` | The primary optimization target |
| **Accuracy** | `(TP + TN) / total` | Overall correctness |
| **F1** | `2 * (precision * recall) / (precision + recall)` | Balance of precision and recall |
| **Regressions** | Count of regression-tier tests that failed | How many previously-correct tests broke after topic refinement |

!!! important "Why Coverage = min(TPR, TNR)"
    A guardrail that catches every violation but also blocks half the safe prompts isn't useful. Coverage forces both detection and specificity to improve together — the system can't game the metric by excelling at one while ignoring the other.

---

## How Tests Work

Each iteration, the LLM generates a balanced test suite:

- **Positive tests** — prompts that _should_ trigger the guardrail (actual violations)
- **Negative tests** — prompts that _should not_ trigger (safe but topically adjacent)

Every test case has four fields:

| Field | What it is |
|-------|-----------|
| `prompt` | The test prompt text |
| `expectedTriggered` | Should the guardrail catch this? |
| `category` | Grouping label (e.g., `"direct-request"`, `"benign-adjacent"`) |
| `source` | How the test entered the suite: `'generated'`, `'carried-fp'`, `'carried-fn'`, or `'regression'` |

### Test Composition (Iteration 2+)

On iteration 2+, the test suite is composed from three sources:

1. **Carried failures** — FP/FN from the previous iteration, re-tested to verify if refinement fixed them
2. **Regression tier** — TP/TN from the previous iteration, re-scanned to catch regressions
3. **Fresh generated** — new tests from the LLM, weighted toward weak categories

All pools are deduplicated case-insensitively. Priority: carried > regression > generated.

### Weighted Category Generation

Per-category error rates from the previous iteration are injected into the test generation prompt. If `"indirect-reference"` had a 40% FN rate vs `"direct-request"` at 5%, the LLM generates more indirect-reference tests.

!!! tip "Why topically adjacent negatives matter"
    The LLM generates negative tests that are _close_ to the guardrail's topic but shouldn't trigger. These are the hardest cases and drive the most improvement — a "weapons" guardrail shouldn't block a cooking discussion about knives.

---

## Scanning

Test prompts are scanned against AIRS in parallel batches:

- **Concurrency**: controlled by `scanConcurrency` (default 5)
- **Detection**: checks `prompt_detected.topic_violation` (fallback: `topic_guardrails_details`)

!!! warning "Rate limits"
    `scanConcurrency` above 5 risks AIRS API throttling. The default balances speed and reliability.

---

## FP/FN Analysis

After scanning, the LLM examines every misclassified result. It receives the topic definition, all test results, and the computed metrics, then identifies patterns:

| Error Type | What happened | Example |
|-----------|--------------|---------|
| **False positive** | Safe prompt incorrectly blocked | Cooking discussion flagged by "weapons" guardrail because "knife" appeared in examples |
| **False negative** | Violation slipped through | Coded language or indirect references not caught by the description |

The analysis produces concrete suggestions — "narrow the description to exclude kitchen contexts" or "add an example covering euphemistic language" — that feed directly into the next iteration.

---

## When the Loop Stops

| Condition | Default | What happens |
|-----------|---------|-------------|
| Coverage target met | 90% | Run succeeds |
| Max iterations reached | 20 | Run completes with best result found |

!!! note
    The best iteration (highest coverage) is tracked throughout the run. Even if the final iteration regresses, the best result is always preserved.
