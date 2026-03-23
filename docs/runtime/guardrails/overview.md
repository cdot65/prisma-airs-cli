---
title: Guardrail Generation
---

# Guardrail Generation

Prisma AIRS CLI's guardrail generation capability uses an LLM-driven feedback loop to create, test, and iteratively refine custom topic guardrails for Prisma AIRS security profiles.

## How It Works

1. **Generate** — An LLM produces a custom topic definition (name, description, examples) based on your intent (block or allow)
2. **Deploy** — The topic is created/updated in AIRS via the Management API and linked to your security profile
3. **Test** — Synthetic test prompts are scanned against the profile to measure detection accuracy
4. **Evaluate** — Metrics (TPR, TNR, coverage, F1) determine how well the guardrail performs
5. **Improve** — The LLM analyzes failures and refines the topic definition
6. **Repeat** — The loop continues until coverage reaches the target threshold (default 90%)

## CLI Usage

Guardrail generation lives under `airs runtime topics`:

```bash
# Interactive mode — prompts for all inputs
airs runtime topics generate

# Non-interactive with all options
airs runtime topics generate \
  --topic "Block discussions about weapons manufacturing" \
  --intent block \
  --profile my-security-profile \
  --target-coverage 90 \
  --max-iterations 5

# Rate-limit scans to avoid API throttling
airs runtime topics generate --rate 10 --topic "..." --profile ...

# Resume, report, list runs
airs runtime topics resume <runId>
airs runtime topics report <runId>
airs runtime topics runs
```

## Key Concepts

- **Intent**: `block` (detect violating prompts) or `allow` (detect benign prompts that should pass through)
- **Coverage**: `min(TPR, TNR)` — both detection types must meet the threshold
- **Topic name lock**: After iteration 1, only the description and examples are refined — the name stays fixed
- **Test composition**: Iteration 2+ carries forward failed tests and adds regression checks alongside fresh LLM-generated tests

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

- [Core Loop Architecture](../../architecture/core-loop.md) — detailed loop state machine
- [Memory System](memory-system.md) — cross-run learning persistence
- [Metrics & Evaluation](metrics.md) — how TP/TN/FP/FN are classified
- [Topic Constraints](topic-constraints.md) — AIRS limits on topic definitions
- [Resumable Runs](resumable-runs.md) — pause and resume loop runs
