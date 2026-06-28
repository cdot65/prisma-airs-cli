---
title: Topic Constraints
---

# Topic Constraints

Prisma AIRS enforces hard limits on custom topic definitions. Prisma AIRS CLI validates these on every `topics create`, rejecting overflow with a clear error so a malformed topic never reaches the AIRS API.

## Limits

| Field | Max Length |
|-------|-----------:|
| **Name** | 100 characters |
| **Description** | 250 characters |
| **Each example** | 250 characters |
| **Examples count** | 2--5 |
| **Combined** (description + all examples) | 1000 characters |

:::warning[UTF-8 Byte Length]
All length checks use UTF-8 byte length, not character count. Multi-byte characters (emoji, non-ASCII) consume more than one unit toward the limit.
:::

---

## Validation on Create

`runtime topics create` runs `validateTopic()` before upserting. It checks every limit and fails fast on any overflow, returning error strings that name exactly what exceeded — it never silently truncates. The agent (or you) fixes the definition and retries.

:::note[Why reject, not clamp]
Surfacing a clear validation error at the CLI boundary keeps the agent loop honest and gives an immediate, actionable message instead of an opaque AIRS API rejection mid-loop.
:::

---

## Where It Lives

`validateTopic()` is exported from `src/core/constraints.ts` and used by the topic commands and test suites. It checks all limits and returns error strings — non-destructive, so it doubles for diagnostics and tests.

---

## Topic Name Locking

:::info
After iteration 1, the topic **name is locked** — only the description and examples change in subsequent iterations. This prevents AIRS sync issues from name changes mid-run.
:::
