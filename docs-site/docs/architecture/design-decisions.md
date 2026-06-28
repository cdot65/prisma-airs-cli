# Design Decisions

The "why" behind Prisma AIRS CLI's architecture. Each decision below was made deliberately — this page explains the trade-offs.

## 1. Atomic CLI Commands

The guardrail workflow uses four atomic commands (`create`, `apply`, `eval`, `revert`) instead of an embedded LLM-driven loop.

**Rationale:** Decouples the optimization intelligence from the CLI. An external agent (Claude Code, etc.) orchestrates the commands following the protocol in [`AGENTS.md`](https://github.com/cdot65/prisma-airs-cli/blob/main/AGENTS.md). Each command succeeds or fails independently, making the workflow recoverable at any point. The CLI is stateless -- no run persistence, no cross-run memory, no embedded LLM calls for guardrail optimization.

:::note[Agent-driven]
The agent provides all intelligence (topic generation, analysis, improvement decisions). The CLI provides only the AIRS API operations and metric computation.
:::

## 2. Topic Name as Identity

The topic name is the stable identity for a guardrail. The external agent picks a name once and keeps it fixed while refining only the description and examples.

**Rationale:** AIRS topics are identified by name. Changing the name would create new entities rather than updating the existing one, leaving orphaned topics and breaking profile references. Keeping the name stable ensures a consistent identity throughout refinement (see Upsert-by-Name below).

## 3. Static Prompt Set Evaluation

The `eval` command scans a static CSV prompt set rather than generating test prompts dynamically via LLM.

**Rationale:** Static prompt sets are deterministic and reproducible. The external agent can curate and evolve the prompt set over time. This removes the LLM dependency from the guardrail optimization loop.

## 4. Config Cascade

Configuration resolves through a strict priority chain:

```
CLI flags  >  Environment variables  >  Config file (~/.prisma-airs/config.json)  >  Zod defaults
```

**Rationale:** A single `ConfigSchema.parse()` call handles validation, coercion, and defaults. No separate validation layer. Users can override any setting at any level without ambiguity about precedence.

:::tip[Home Directory Expansion]
Paths containing `~` are expanded via `expandHome()` during config loading, so `~/.prisma-airs/config.json` works on all platforms.
:::

## 5. Constraint Validation on Create

`runtime topics create` validates every topic against AIRS hard limits via `validateTopic()` before upserting, rejecting anything that overflows rather than silently truncating it.

**Rationale:** The external agent supplies topic definitions; surfacing a clear validation error keeps the agent's loop honest and prevents AIRS from rejecting a malformed topic later. The enforced limits:

| Constraint | Limit |
|-----------|-------|
| Topic name | 100 characters |
| Description | 250 characters |
| Each example | 250 characters |
| Max examples | 5 |
| Combined (description + all examples) | 1000 characters |

:::tip[Fail Fast]
Validation happens at the CLI boundary, so the agent gets an immediate, actionable error instead of an opaque AIRS API rejection mid-loop.
:::

## 6. Upsert-by-Name Semantics

The `create` command upserts topics by name rather than requiring separate create/update paths.

**Rationale:** The external agent doesn't need to track topic IDs. It specifies the topic by name, and the CLI handles create-vs-update internally. This simplifies the agent loop protocol.

## 7. External Agent Orchestration

The CLI provides atomic operations; an external agent provides the intelligence and orchestration.

**Rationale:** Embedding the LLM loop inside the CLI created tight coupling between the optimization strategy and the CLI tool. By extracting the loop to an external agent (defined in [`AGENTS.md`](https://github.com/cdot65/prisma-airs-cli/blob/main/AGENTS.md)), the optimization strategy can evolve independently. Different agents can use the same CLI commands with different strategies.

## 8. Intent-Aware Refinement (Agent Responsibility)

Refinement intelligence lives in the external agent, not the CLI. The agent decides how to evolve a topic between `eval` runs, and `block` vs `allow` intent flips the error priority it should optimize for:

| Intent | High Severity Error | Strategy |
|--------|-------------------|----------|
| `block` (blacklist) | False Negatives — dangerous content slipping through | Widen coverage, broaden examples |
| `allow` (whitelist) | False Positives — blocking legitimate conversations | Tighten precision, sharpen description |

Treating every guardrail as block-style (catch more) actively harms allow guardrails by making them over-trigger, so the agent must carry the intent through its own loop.

:::note[Example Count (2-5)]
AIRS requires a minimum of 2 examples and allows up to 5. The description field carries the most weight in topic matching, so fewer, sharper examples often outperform many broad ones — a trade-off the agent tunes per topic.
:::

## 9. CSV Prompt Sets

The `eval` command accepts CSV prompt sets with `prompt` and `expectedTriggered` columns.

**Rationale:** CSV is simple, diffable, and easy to generate. The external agent or a human can curate prompt sets outside the CLI. This replaces LLM-generated test prompts with deterministic, reproducible evaluation.

:::info[Summary]
The common thread across these decisions is **separation of concerns**: the CLI provides atomic AIRS operations, the agent provides intelligence and orchestration, and the config system resolves settings. Each subsystem is independently testable and replaceable.
:::
