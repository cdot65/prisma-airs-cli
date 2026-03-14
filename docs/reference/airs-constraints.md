# AIRS Constraints

Prisma AIRS enforces hard limits on custom topic definitions. Prisma AIRS CLI validates and auto-clamps topics to fit within these boundaries before every API call.

---

## Limits

| Constraint | Limit |
|-----------|------:|
| Topic name | 100 characters |
| Description | 250 characters |
| Each example | 250 characters |
| Number of examples | 2--5 |
| Combined total (description + all examples) | 1000 characters |

!!! warning "Byte length"
    The combined total uses **UTF-8 byte length**, not character count. Multi-byte characters (emoji, CJK, accented letters) consume more than one unit toward the 1000 limit.

---

## Automatic Clamping

`clampTopic()` enforces limits after every LLM response:

1. **Truncate name** to 100 characters
2. **Truncate each example** to 250 characters
3. **If combined > 1000**: drop trailing examples one at a time
4. **If still over**: truncate description to fit remaining budget

```
LLM output → clampTopic() → AIRS-compliant topic
```

### Why Post-LLM Clamping?

The LLM routinely exceeds the 250-char description limit when writing natural language. Rejecting the entire response via Zod validation would force expensive retries. Post-processing lets the LLM generate freely while guaranteeing AIRS compatibility on every call.

!!! note
    Constraints are defined in `src/core/constraints.ts` and imported by the LLM service and test suites.

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

!!! danger "Never reference profiles by UUID"
    Each profile update generates a new revision with a new UUID. Storing or referencing a profile by UUID will break on the next update. Use the profile **name** as the stable identifier.
