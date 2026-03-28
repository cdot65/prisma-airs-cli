# Design Decisions

The "why" behind Prisma AIRS CLI's architecture. Each decision below was made deliberately — this page explains the trade-offs.

## 1. Atomic CLI Commands

The guardrail workflow uses four atomic commands (`create`, `apply`, `eval`, `revert`) instead of an embedded LLM-driven loop.

**Rationale:** Decouples the optimization intelligence from the CLI. An external agent (Claude Code, Gemini CLI, etc.) orchestrates the commands following the protocol in `program.md`. Each command succeeds or fails independently, making the workflow recoverable at any point. The CLI is stateless -- no run persistence, no cross-run memory, no embedded LLM calls for guardrail optimization.

!!! note "Agent-driven"
    The agent provides all intelligence (topic generation, analysis, improvement decisions). The CLI provides only the AIRS API operations and metric computation.

## 2. Topic Name Locking

The topic name is generated in iteration 1 and locked for all subsequent iterations. Only the description and examples are refined.

**Rationale:** AIRS topics are identified by name. Changing the name each iteration would create new entities rather than updating the existing one, leaving orphaned topics and breaking profile references. Locking the name ensures a stable identity throughout the refinement process.

## 3. Static Prompt Set Evaluation

The `eval` command scans a static CSV prompt set rather than generating test prompts dynamically via LLM.

**Rationale:** Static prompt sets are deterministic and reproducible. The external agent can curate and evolve the prompt set over time. This removes the LLM dependency from the guardrail optimization loop (LLM is still used for profile audits).

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

## 6. Upsert-by-Name Semantics

The `create` command upserts topics by name rather than requiring separate create/update paths.

**Rationale:** The external agent doesn't need to track topic IDs. It specifies the topic by name, and the CLI handles create-vs-update internally. This simplifies the agent loop protocol.

## 7. Structured Output via Zod

All four LLM calls use LangChain's `withStructuredOutput(ZodSchema)` with 3 retries on parse failure.

**Rationale:** Structured output guarantees type-safe responses at the boundary between the LLM and the application. The retry mechanism handles occasional malformed JSON from the model without failing the entire iteration. Zod schemas serve double duty as both runtime validators and TypeScript type sources.

```typescript
const chain = llm.withStructuredOutput(TopicSchema, {
  name: "generate_topic",
});
// Returns a typed CustomTopic or throws after 3 retries
```

## 8. External Agent Orchestration

The CLI provides atomic operations; an external agent provides the intelligence and orchestration.

**Rationale:** Embedding the LLM loop inside the CLI created tight coupling between the optimization strategy and the CLI tool. By extracting the loop to an external agent (defined in `program.md`), the optimization strategy can evolve independently. Different agents can use the same CLI commands with different strategies.

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

## 10. CSV Prompt Sets

The `eval` command accepts CSV prompt sets with `prompt` and `expectedTriggered` columns.

**Rationale:** CSV is simple, diffable, and easy to generate. The external agent or a human can curate prompt sets outside the CLI. This replaces LLM-generated test prompts with deterministic, reproducible evaluation.

!!! abstract "Summary"
    The common thread across these decisions is **separation of concerns**: the CLI provides atomic AIRS operations, the agent provides intelligence and orchestration, and the config system resolves settings. Each subsystem is independently testable and replaceable.
