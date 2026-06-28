# AIRS Constraints

Prisma AIRS enforces hard limits on custom topic definitions. Prisma AIRS CLI validates topics against these boundaries before every API call, rejecting anything that overflows rather than silently truncating it.

---

## Limits

| Constraint | Limit |
|-----------|------:|
| Topic name | 100 characters |
| Description | 250 characters |
| Each example | 250 characters |
| Number of examples | 2--5 |
| Combined total (description + all examples) | 1000 characters |

:::warning[Byte length]
The combined total uses **UTF-8 byte length**, not character count. Multi-byte characters (emoji, CJK, accented letters) consume more than one unit toward the 1000 limit.
:::

---

## Validation on Create

`runtime topics create` runs `validateTopic()` before upserting. It checks every limit and, on any overflow, fails with a clear error listing what exceeded — it never silently truncates:

1. **Name** must be ≤ 100 characters
2. **Each example** must be ≤ 250 characters
3. **Description** must be ≤ 250 characters
4. **Combined** (description + all examples) must be ≤ 1000 characters

### Why Reject Instead of Truncate?

Surfacing a clear validation error at the CLI boundary keeps the agent loop honest and gives an immediate, actionable message instead of an opaque AIRS API rejection later. The agent fixes the definition and retries.

:::note
Constraints and `validateTopic()` are defined in `src/core/constraints.ts` and imported by the topic commands and test suites.
:::

---

## Profile Integration

Topics are deployed into an AIRS security profile:

```
profile
└── model-protection
    └── topic-guardrails
        └── topic-list
            └── [your custom topic]
```

### Key Behaviors

- **Profile updates create new revisions** with new UUIDs. Always reference profiles by **name**, never by ID.
- **Topics can't be deleted** while referenced by any profile revision.

:::danger[Never reference profiles by UUID]
Each profile update generates a new revision with a new UUID. Storing or referencing a profile by UUID will break on the next update. Use the profile **name** as the stable identifier.
:::
