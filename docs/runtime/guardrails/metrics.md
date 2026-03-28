---
title: Metrics & Evaluation
---

# Metrics & Evaluation

The `eval` command measures how well a guardrail performs against a static prompt set. Here's what gets measured and why.

## The Metrics

| Metric | Formula | What it tells you |
|--------|---------|-------------------|
| **TPR** (sensitivity) | `TP / (TP + FN)` | How many violations are caught |
| **TNR** (specificity) | `TN / (TN + FP)` | How many safe prompts are correctly passed |
| **Coverage** | `min(TPR, TNR)` | The primary optimization target |
| **Accuracy** | `(TP + TN) / total` | Overall correctness |
| **F1** | `2 * (precision * recall) / (precision + recall)` | Balance of precision and recall |
| **Regressions** | Count of previously-correct tests that now fail | How many tests broke after topic refinement |

!!! important "Why Coverage = min(TPR, TNR)"
    A guardrail that catches every violation but also blocks half the safe prompts isn't useful. Coverage forces both detection and specificity to improve together — the system can't game the metric by excelling at one while ignoring the other.

---

## How Tests Work

The `eval` command reads a static CSV prompt set with two required columns:

- **`prompt`** — the test prompt text
- **`expectedTriggered`** — `true` if the guardrail should catch this, `false` if it should pass

The prompt set should include both:

- **Positive tests** — prompts that _should_ trigger the guardrail (actual violations)
- **Negative tests** — prompts that _should not_ trigger (safe but topically adjacent)

!!! tip "Why topically adjacent negatives matter"
    Include negative tests that are _close_ to the guardrail's topic but shouldn't trigger. These are the hardest cases — a "weapons" guardrail shouldn't block a cooking discussion about knives.

---

## Scanning

Test prompts are scanned against AIRS in parallel batches:

- **Concurrency**: controlled by `scanConcurrency` (default 5)
- **Detection**: checks `prompt_detected.topic_violation` (fallback: `topic_guardrails_details`)

!!! warning "Rate limits"
    `scanConcurrency` above 5 risks AIRS API throttling. The default balances speed and reliability.

---

## FP/FN Details

The `eval` command returns details for every misclassified result:

| Error Type | What happened | Example |
|-----------|--------------|---------|
| **False positive** | Safe prompt incorrectly blocked | Cooking discussion flagged by "weapons" guardrail because "knife" appeared in examples |
| **False negative** | Violation slipped through | Coded language or indirect references not caught by the description |

The external agent uses these details to refine the topic definition in the next iteration.
