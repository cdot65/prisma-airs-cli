# Core Loop

The heart of Prisma AIRS CLI. The core loop (`src/core/loop.ts`) is an async generator that yields typed events as it works. The CLI renders those events, but the loop itself has no knowledge of how its output is displayed — making it independently testable and reusable.

## What Happens Each Iteration

Each iteration follows a fixed sequence:

```mermaid
flowchart TD
    Start([Iteration Start]) --> GenOrImprove{Iteration 1?}
    GenOrImprove -->|Yes| Generate[Generate Topic via LLM]
    GenOrImprove -->|No| Improve[Improve Topic via LLM]
    Generate --> Clamp[clampTopic - enforce AIRS limits]
    Improve --> Clamp
    Clamp --> Deploy[Deploy via Management API]
    Deploy --> TestGen[Generate Test Cases via LLM]
    TestGen --> Scan[Batch Scan via Scanner API]
    Scan --> Metrics[Compute Metrics - TPR, TNR, Coverage, F1]
    Metrics --> Analyze[Analyze FP/FN Patterns via LLM]
    Analyze --> Check{Coverage >= Target?}
    Check -->|Yes| Complete([Loop Complete])
    Check -->|No| MaxCheck{Max Iterations?}
    MaxCheck -->|Yes| Complete
    MaxCheck -->|No| Start
```

## Events

The generator yields events at each stage. Consumers (like the CLI renderer) iterate the generator and switch on the event `type`.

### Yielded by `runLoop()`

| Event | Payload | When |
|-------|---------|------|
| `iteration:start` | iteration number | Start of each iteration |
| `generate:complete` | `CustomTopic` | After LLM generates or improves topic |
| `apply:complete` | topic ID | After topic deployed to AIRS (yielded but intentionally unhandled in CLI) |
| `tests:composed` | generated, carried failures, regression tier, total | After test suite composed from generated + carried FP/FN + regression tier (iteration 2+) |
| `tests:accumulated` | new count, total count, dropped count | After test accumulation merges new + old tests (only when `accumulateTests` enabled, iteration 2+) |
| `test:progress` | completed count, total | Per-test scan completion |
| `evaluate:complete` | `EfficacyMetrics` | After metrics computed |
| `analyze:complete` | `AnalysisReport` | After FP/FN analysis |
| `iteration:complete` | `IterationResult` | Full iteration summary |
| `memory:extracted` | learning count | Learnings extracted post-loop (only if memory enabled) |
| `loop:complete` | best iteration, run state | Terminal: target reached or max iterations |

### Defined but not yielded by `runLoop()`

| Event | Payload | Status |
|-------|---------|--------|
| `loop:paused` | current state | Reserved for future use — not currently yielded |
| `memory:loaded` | learning count | Emitted by CLI command (`generate.ts`) before the loop starts, not by the generator itself |

!!! tip "Terminal Events"
    `loop:complete` is the terminal event. After it is yielded, the generator returns and no further events are produced. `loop:paused` is defined in the type union for future use but is not currently yielded.

## Topic Name Locking

The topic name is generated once during **iteration 1** and locked for all subsequent iterations. Only the description and examples are refined in later iterations.

This prevents two problems:

- **Identity thrashing** -- changing the topic name on each iteration would create new AIRS entities instead of updating the existing one.
- **Entity inconsistency** -- downstream profile references depend on a stable topic identity.

!!! warning "Name Immutability"
    The loop enforces name locking internally. Even if the LLM returns a different name in its improvement output, the original name from iteration 1 is preserved.

## Stop Conditions

The loop terminates when either condition is met:

| Condition | Default | Description |
|-----------|---------|-------------|
| Coverage target reached | `0.9` (90%) | `coverage = min(TPR, TNR)` must meet or exceed `targetCoverage` |
| Max iterations exceeded | `20` | Hard upper bound on refinement cycles |

!!! info "Coverage Definition"
    Coverage is defined as `min(TPR, TNR)`, not a simple average. This ensures both true-positive and true-negative performance must reach the target -- the system cannot pass by excelling at one while failing the other.

## Four LLM Calls Per Iteration

Each iteration makes up to four LLM calls, all using `withStructuredOutput(ZodSchema)`:

1. **Generate / Improve Topic** -- produces a `CustomTopic` (name, description, examples)
2. **Generate Test Cases** -- produces positive and negative test prompts
3. **Analyze Results** -- examines false positives and false negatives for patterns (intent-aware: prioritizes FN reduction for block, FP reduction for allow)
4. *(Post-loop)* **Extract Learnings** -- distills iteration history into reusable memory entries

## Test Composition

On iteration 2+, the test suite is automatically composed from three sources:

1. **Carried failures** (always-on): FP and FN test cases from the previous iteration are re-tested to verify whether topic refinement resolved them. Tagged with `source: 'carried-fp'` or `'carried-fn'`.
2. **Regression tier** (always-on): TP and TN (correct) test cases from the previous iteration are re-scanned. If they now fail, that's a regression. Tagged with `source: 'regression'`.
3. **Fresh generated tests**: New tests from the LLM, informed by per-category error rates from the previous iteration (weighted generation). Tagged with `source: 'generated'`.

All three pools are deduplicated case-insensitively by prompt text. Priority: carried failures > regression > generated.

The `tests:composed` event reports the breakdown on each iteration 2+.

### Weighted Category Generation

On iteration 2+, `computeCategoryBreakdown()` computes per-category FP/FN error rates from the previous iteration's results. This breakdown is injected into the LLM test generation prompt, instructing it to generate proportionally more tests for weak categories.

### Regression Tracking

`EfficacyMetrics.regressionCount` counts regression-tier tests that failed (previously correct, now wrong after topic refinement). Regressions also count in the normal FP/FN tallies — the separate counter surfaces _how many_ failures were caused by topic changes vs. being new failures.

## Test Accumulation (Legacy)

The `accumulateTests` flag enables additional full-pool accumulation on top of the composition logic. When enabled, tests from all prior iterations are also merged (not just the previous iteration's failures and regressions):

- **Deduplication**: case-insensitive by prompt text, new tests take priority over old
- **Max cap**: optional `maxAccumulatedTests` limits total count, keeping newest first
- **Event**: `tests:accumulated` is yielded on iterations 2+ with new/total/dropped counts

```typescript
const input: UserInput = {
  // ...
  accumulateTests: true,
  maxAccumulatedTests: 50, // optional cap
};
```
