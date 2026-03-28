---
title: Guardrail Optimization
---

# Guardrail Optimization

Prisma AIRS CLI provides atomic commands for creating, testing, and iteratively refining custom topic guardrails. An external agent (Claude Code, Gemini CLI, etc.) orchestrates these commands in a loop following the protocol in `program.md`.

## How It Works

1. **Create** — Define a custom topic (name, description, examples) with an intent (block or allow)
2. **Apply** — Assign the topic to a security profile (additive, preserves existing topics)
3. **Eval** — Scan a static CSV prompt set against the profile, compute metrics (TPR, TNR, coverage, F1), return FP/FN details
4. **Decide** — The agent analyzes results and decides to keep or revert
5. **Revert** (if needed) — Remove the topic from the profile and delete it
6. **Repeat** — The agent refines the topic definition and tries again

## CLI Usage

Guardrail optimization lives under `airs runtime topics`:

```bash
# Create or update a topic (upserts by name)
airs runtime topics create --topic "Block weapons manufacturing" --intent block

# Assign topic to a profile
airs runtime topics apply --profile my-security-profile --topic "Weapons Manufacturing"

# Evaluate against a prompt set
airs runtime topics eval --profile my-security-profile --input prompts.csv

# Revert if metrics regressed
airs runtime topics revert --profile my-security-profile --topic "Weapons Manufacturing"
```

## Key Concepts

- **Intent**: `block` (detect violating prompts) or `allow` (detect benign prompts that should pass through)
- **Coverage**: `min(TPR, TNR)` — both detection types must meet the threshold
- **Upsert by name**: `create` updates an existing topic if one with the same name exists
- **Static prompt sets**: `eval` uses CSV files with `prompt` and `expectedTriggered` columns for deterministic evaluation

## Platform Constraints

Achievable coverage depends on the topic domain and intent. AIRS has platform-level behaviors that limit what custom topic guardrails can accomplish.

### Block-Intent on High-Sensitivity Topics

Certain topic domains (explosives/weapons, CSAM, etc.) trigger AIRS built-in safety layers that **override custom topic definitions entirely**:

- These topics achieve 100% TPR but **0% TNR** — the guardrail blocks ALL content, including completely unrelated prompts
- Description refinement, exclusion clauses, and example tuning have zero observable effect
- The built-in safety layer appears to key off the topic name/domain, not the description

!!! warning "Recommendation"
    Do not create custom block-intent topics for content that AIRS already handles via built-in safety. Use the default AIRS security profiles instead.

### Allow-Intent Matching Behavior

Allow-intent matching uses **broad semantic similarity**, not logical constraint evaluation:

- Exclusion clauses ("not X", "excludes Y") do not work — they often **increase** false positives by adding semantic overlap with the excluded domain
- Shorter, simpler descriptions (under 100 characters) consistently outperform longer, more specific ones
- Typical achievable coverage for allow-intent topics: **40–70%** depending on topic breadth
- Best results usually come from the first few iterations; extended refinement often degrades coverage

### Description Truncation

AIRS enforces hard limits on topic definitions. Descriptions exceeding 250 characters are silently truncated by `clampTopic()`, which can strip the positive definition while preserving only exclusion clauses — further degrading performance.

| Constraint | Limit |
|-----------|-------|
| Topic name | 100 characters |
| Description | 250 characters |
| Each example | 250 characters |
| Max examples | 5 |
| Combined (description + examples) | 1000 characters |

## Related

- [Guardrail Optimization Architecture](../../architecture/core-loop.md) — command cycle and design decisions
- [Metrics & Evaluation](metrics.md) — how TP/TN/FP/FN are classified
- [Topic Constraints](topic-constraints.md) — AIRS limits on topic definitions
- `program.md` — full agent loop protocol
