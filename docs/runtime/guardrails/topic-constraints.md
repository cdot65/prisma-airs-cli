---
title: Topic Constraints
---

# Topic Constraints

Prisma AIRS enforces hard limits on custom topic definitions. Prisma AIRS CLI handles these automatically so you don't have to worry about truncation or rejection.

## Limits

| Field | Max Length |
|-------|-----------:|
| **Name** | 100 characters |
| **Description** | 250 characters |
| **Each example** | 250 characters |
| **Examples count** | 2--5 |
| **Combined** (description + all examples) | 1000 characters |

!!! warning "UTF-8 Byte Length"
    All length checks use UTF-8 byte length, not character count. Multi-byte characters (emoji, non-ASCII) consume more than one unit toward the limit.

---

## Automatic Clamping

The LLM frequently exceeds the 250-character description limit — natural language tends to be verbose. The `clampTopic()` function enforces limits automatically after every LLM call.

The strategy is ordered by impact: drop trailing examples first, truncate the description only as a last resort. This preserves as much semantic content as possible.

!!! note "Why post-LLM, not Zod"
    Clamping happens _after_ the LLM generates output, not via Zod schema validation. The LLM needs freedom to produce natural descriptions, and the priority-based truncation logic is more nuanced than schema constraints can express.

---

## Validation vs. Clamping

`src/core/constraints.ts` exports two approaches:

| Function | What it does |
|----------|-------------|
| **Validation** | Checks all limits, returns error strings. Non-destructive — useful for diagnostics and tests. |
| **Clamping** | Silently enforces all limits by truncating and dropping as needed. Used in the production pipeline. |

---

## Topic Name Locking

!!! info
    After iteration 1, the topic **name is locked** — only the description and examples change in subsequent iterations. This prevents AIRS sync issues from name changes mid-run.
