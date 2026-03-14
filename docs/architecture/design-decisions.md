# Design Decisions

The "why" behind Prisma AIRS CLI's architecture. Each decision below was made deliberately — this page explains the trade-offs.

## 1. Async Generator Loop

`runLoop()` is an async generator that yields typed `LoopEvent` discriminated unions rather than calling renderers or side-effecting functions directly.

**Rationale:** Decouples the iteration engine from the UI layer. The CLI iterates the generator and dispatches events to its Chalk-based renderer, but the loop itself has no knowledge of how events are consumed. This makes the entire loop testable with mock event consumers -- no mocking of terminal output required.

```typescript
for await (const event of runLoop(services, config)) {
  renderer.handle(event); // CLI dispatches here; tests collect events instead
}
```

!!! note "Swap-friendly"
    A web UI or API server could consume the same generator without changing any core loop code.

## 2. Topic Name Locking

The topic name is generated in iteration 1 and locked for all subsequent iterations. Only the description and examples are refined.

**Rationale:** AIRS topics are identified by name. Changing the name each iteration would create new entities rather than updating the existing one, leaving orphaned topics and breaking profile references. Locking the name ensures a stable identity throughout the refinement process.

## 3. Budget-Aware Memory Injection

Memory injection uses a character budget (default 3000, configurable 500--10000) instead of a hard item count.

**Rationale:** A fixed count cap treats all learnings equally regardless of length. Budget-based injection prioritizes high-value content:

| Priority | Format | Description |
|----------|--------|-------------|
| Highest | Verbose | Full learning text with examples; used for top-corroborated learnings |
| Medium | Compact | Shortened summary; used when budget is tight |
| Lowest | Omitted | Excluded entirely; count appended as "and N more learnings..." |

Learnings are sorted by corroboration count (descending) before budget allocation. This ensures battle-tested insights always make it into the prompt.

## 4. Config Cascade

Configuration resolves through a strict priority chain:

```
CLI flags  >  Environment variables  >  Config file (~/.prisma-airs/config.json)  >  Zod defaults
```

**Rationale:** A single `ConfigSchema.parse()` call handles validation, coercion, and defaults. No separate validation layer. Users can override any setting at any level without ambiguity about precedence.

!!! tip "Home Directory Expansion"
    Paths containing `~` are expanded via `expandHome()` during config loading, so `~/.prisma-airs/config.json` works on all platforms.

## 5. Post-LLM Clamping

LLM output is clamped to AIRS constraints *after* generation rather than relying solely on Zod schema validation.

**Rationale:** LLMs routinely exceed the 250-character AIRS description limit despite prompt instructions. `clampTopic()` enforces hard limits:

| Constraint | Limit |
|-----------|-------|
| Topic name | 100 characters |
| Description | 250 characters |
| Each example | 250 characters |
| Max examples | 5 |
| Combined (description + all examples) | 1000 characters |

The clamping strategy is ordered: drop trailing examples first if the combined limit is exceeded, then trim the description as a last resort. This preserves as much semantic content as possible.

!!! warning "Why Not Zod Alone?"
    Zod `.max()` would reject the entire response on overflow, requiring a full retry. Clamping is cheaper and deterministic -- it always produces a valid topic on the first pass.

## 6. Category-Based Memory

Learnings are stored in files keyed by a normalized category derived from the topic's keywords.

**Normalization pipeline:**

1. Lowercase all keywords
2. Strip punctuation
3. Remove stop words
4. Sort alphabetically
5. Join with hyphens

**Cross-topic transfer** occurs when two categories share 50% or more keyword overlap. This allows learnings from "api-injection-sql" to inform a run targeting "injection-prompt-sql" without requiring exact matches.

**Rationale:** File-per-category keeps I/O simple (no database) while keyword overlap enables knowledge transfer across related topics without manual tagging.

## 7. Structured Output via Zod

All four LLM calls use LangChain's `withStructuredOutput(ZodSchema)` with 3 retries on parse failure.

**Rationale:** Structured output guarantees type-safe responses at the boundary between the LLM and the application. The retry mechanism handles occasional malformed JSON from the model without failing the entire iteration. Zod schemas serve double duty as both runtime validators and TypeScript type sources.

```typescript
const chain = llm.withStructuredOutput(TopicSchema, {
  name: "generate_topic",
});
// Returns a typed CustomTopic or throws after 3 retries
```

## 8. Event-Driven Architecture

The `LoopEvent` union defines 12 event types. Of these, 10 are yielded by `runLoop()`:

| Phase | Events |
|-------|--------|
| Per-iteration | `iteration:start`, `generate:complete`, `apply:complete`, `test:progress`, `evaluate:complete`, `analyze:complete`, `iteration:complete` |
| Post-loop | `memory:extracted` (if memory enabled), `promptset:created` (if `--create-prompt-set`) |
| Terminal | `loop:complete` |

Two events are defined in the type union but not yielded by the loop:

| Event | Status |
|-------|--------|
| `memory:loaded` | Emitted by CLI command before the loop starts |
| `loop:paused` | Reserved for future use |

**Rationale:** Fine-grained events enable rich progress reporting (the CLI shows per-test scan progress), clean separation of concerns (renderer knows nothing about LLM calls), and future extensibility (logging, metrics dashboards, web UIs) without modifying the core loop.

## 9. Intent-Aware Refinement

The `analyzeResults()` and `improveTopic()` LLM calls receive the guardrail intent (`"block"` or `"allow"`) as a prompt variable.

**Rationale:** Block and allow guardrails have opposite error priorities:

| Intent | High Severity Error | Strategy |
|--------|-------------------|----------|
| `block` (blacklist) | False Negatives — dangerous content slipping through | Widen coverage, broaden examples |
| `allow` (whitelist) | False Positives — blocking legitimate conversations | Tighten precision, sharpen description |

Without intent context, the LLM defaults to block-style refinement (catch more), which actively harms allow guardrails by making them over-trigger.

### Allow-Intent Detection via `category`

AIRS reports topic detection differently for allow vs block intent:

| Intent | Prompt matches topic | `triggered` | `category` |
|--------|---------------------|-------------|------------|
| Block | Yes (violating content) | `true` | `malicious` |
| Block | No (benign content) | `false` | `benign` |
| Allow | Yes (permitted content) | `false` | `benign` |
| Allow | No (non-permitted content) | `false` | `malicious` |

For allow intent, `triggered` is never `true`. The `action` field is also unreliable (always `allow`). The `category` field is the correct discriminator: `"benign"` means the content matched the allow topic, `"malicious"` means it did not. The loop uses `category === 'benign'` for allow-intent detection, falling back to `triggered` when `category` is absent.

### Variable Example Count (2-5)

The LLM is instructed to vary example count between 2-5 across iterations. The AIRS API requires a minimum of 2 examples. The description field carries the most weight in AIRS topic matching, so fewer, sharper examples often outperform many broad ones. The memory system tracks example count per iteration and extracts learnings about which counts correlate with better efficacy.

## 10. Optional Test Accumulation

When `accumulateTests` is enabled, test prompts carry forward across iterations instead of being regenerated fresh each time. New tests take priority during deduplication (case-insensitive, by prompt text). An optional `maxAccumulatedTests` cap limits growth.

**Rationale:** Fresh test generation each iteration can miss regression detection — a fix for false negatives might introduce new false positives that go undetected because the triggering prompts were only present in the previous iteration's test set. Accumulation ensures previously-failing prompts remain in the test pool.

!!! tip "Cap Behavior"
    When `maxAccumulatedTests` is set, the newest tests are kept and oldest are dropped. This prevents unbounded growth while preserving the most relevant test cases.

## 11. Custom Prompt Set Export

When `--create-prompt-set` is passed, the loop auto-creates a custom prompt set in AI Runtime Security's Red Team module using the best iteration's test cases.

**Rationale:** The test prompts generated during refinement are high-quality, topic-specific attack and benign prompts. Exporting them as a reusable prompt set closes the loop — Prisma AIRS CLI generates guardrails AND the test assets to validate them in production. This also validates the Management SDK's `RedTeamClient.customAttacks` API end-to-end.

**Implementation:** After `loop:complete` is determined but before the event is yielded, the loop creates a prompt set via `PromptSetService.createPromptSet()`, then adds each test case as a prompt with a goal indicating whether it should trigger the guardrail.

!!! abstract "Summary"
    The common thread across these decisions is **separation of concerns**: the loop generates events, the renderer displays them, the memory system persists learnings, and the config system resolves settings. Each subsystem is independently testable and replaceable.
