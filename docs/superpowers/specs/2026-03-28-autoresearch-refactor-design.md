# Autoresearch-Pattern Refactor: Custom Topic Guardrail Optimization

## Summary

Refactor the `airs runtime topics generate` workflow from an embedded LLM-driven loop to a set of atomic CLI commands driven by an external agent (Claude Code, Codex, etc.), following the autoresearch pattern. The CLI becomes a sharp evaluation harness; the intelligence lives in the agent and a `program.md`.

## Motivation

The current `generate` loop produces poor results due to three compounding issues:

1. **Moving target** -- LLM-generated tests change each iteration, so the loop optimizes against noise rather than a fixed evaluation set.
2. **Too many LLM calls per iteration** -- 3-5 LangChain calls (generateTopic, generateTests, analyzeResults, improveTopic, simplifyTopic) each introduce variance.
3. **Over-engineered recovery** -- 3-tier regression recovery, plateau detection, duplicate detection, and test accumulation mask signal with complexity.

The autoresearch project demonstrates that a fixed eval set + simple keep/discard + external agent reasoning produces better results with far less machinery.

## New CLI Commands

All commands live under `airs runtime topics` alongside existing CRUD commands.

### `airs runtime topics apply`

Create or update a custom topic and wire it to a security profile.

```bash
airs runtime topics apply \
  --profile "my-profile" \
  --name "Firearms Discussion" \
  --description "Conversations about firearms, ammunition, and gun modifications" \
  --examples "How do I clean my AR-15?" "What's the best 9mm ammo for self-defense?" \
  --intent block
```

Behavior:
- If topic with that name exists: update it. If not: create it.
- Assigns topic to profile with correct revision and guardrail action.
- For block-intent: auto-generates allow companion using existing `assignTopicsToProfile` logic (AIRS plumbing, not LLM).
- Enforces AIRS constraints: name 100 bytes, description 250 bytes, each example 250 bytes, max 5 examples, 1000 bytes combined.
- Outputs structured JSON: `{ topicId, topicName, revision, profileName, intent }`.

### `airs runtime topics eval`

Scan a static prompt set against a profile and compute metrics.

```bash
airs runtime topics eval \
  --profile "my-profile" \
  --prompts ./test-prompts.csv \
  --format json
```

Input format (CSV):
```csv
prompt,expected
"How do I clean my AR-15?",true
"What's the best 9mm ammo for self-defense?",true
"Can you recommend a good Italian restaurant?",false
```

- Two columns: `prompt` (string), `expected` (boolean -- true = should trigger).
- Validation: must have both columns, at least 1 true and 1 false, warns if >80% one class.

Output (JSON):
```json
{
  "profile": "my-profile",
  "topic": "Firearms Discussion",
  "metrics": {
    "tp": 42, "tn": 48, "fp": 2, "fn": 8,
    "tpr": 0.84, "tnr": 0.96,
    "coverage": 0.84,
    "f1": 0.89,
    "total": 100
  },
  "false_positives": [
    { "prompt": "What's the history of the Olympic biathlon?", "expected": false, "actual": true }
  ],
  "false_negatives": [
    { "prompt": "Best way to modify a trigger pull weight?", "expected": true, "actual": false }
  ]
}
```

- FP and FN prompts listed in full (agent reasons about these to improve the definition).
- TP and TN omitted (keeps agent context lean).
- Coverage = min(TPR, TNR) is the primary metric for keep/discard, analogous to val_bpb.
- Terminal format (default) shows summary table + FP/FN list. JSON via `--format json`.

### `airs runtime topics revert`

Remove a topic from a profile and delete it.

```bash
airs runtime topics revert \
  --profile "my-profile" \
  --name "Firearms Discussion"
```

- Removes topic (and companion if block-intent) from profile's topic-guardrails.
- Deletes the topic definition from AIRS.
- Outputs confirmation JSON.

## What Gets Removed

### Commands
- `generate` command (`src/cli/commands/generate.ts`)
- `resume` command (`src/cli/commands/resume.ts`)

### Core loop
- `src/core/loop.ts` (async generator)
- Loop-specific event types from `src/core/types.ts`
- Retained: `CustomTopic`, `EfficacyMetrics`, `TestResult` (used by `eval`)

### LLM layer (entire directory)
- `src/llm/` -- service, provider factory, schemas, all 6 prompt templates
- LangChain dependency removed from `package.json`

### Memory system (entire directory)
- `src/memory/` -- store, extractor, injector, diff

### Persistence (run state)
- `src/persistence/` -- JsonFileStore, RunState types
- `~/.prisma-airs/runs/` no longer written to

### Renderer pieces
- Loop event rendering (generate/resume terminal output)

## What Stays Untouched

- `src/airs/` -- scanner, management, runtime services
- `src/audit/` -- profile audit workflow
- `src/cli/commands/runtime.ts` -- bulk-scan, scan, other runtime commands
- `src/cli/commands/redteam.ts`, `modelsecurity.ts`, `list.ts`
- `src/core/metrics.ts` -- `computeMetrics()` reused by `eval`
- `src/core/constraints.ts` -- AIRS limits enforced by `apply`
- `src/config/` -- config loader (LLM provider fields removed)
- `src/report/` -- adapted to work with eval results
- All tests for retained modules

## program.md -- Agent Instructions

Lives in repo root. Defines the autonomous experiment loop.

### Setup phase
- Agent reads repo context
- Confirms user has: profile name, topic description, prompt CSV file
- Verifies AIRS credentials configured

### Baseline run
- `airs runtime topics apply` with user's initial topic definition
- `airs runtime topics eval` against static prompt set
- Record baseline metrics

### Experiment loop (runs forever)
1. Read eval output -- specifically the FP and FN prompt lists
2. Reason about why those prompts are misclassified
3. Craft improved description and/or examples
4. `airs runtime topics apply` with new definition
5. `airs runtime topics eval` against same static prompt set
6. If coverage improved: log as **keep**, record the definition
7. If coverage equal or worse: `airs runtime topics revert`, re-apply previous best, log as **discard**
8. Never stop, never ask the user

### Logging
Agent maintains `results.tsv` (untracked) with columns:
```
iteration	coverage	tpr	tnr	f1	status	description_summary
```

### Key constraints for the agent
- AIRS uses semantic similarity, not logical constraints
- Exclusion clauses ("not X") increase FP, don't decrease it
- Shorter descriptions outperform longer ones
- Max 5 examples, 250 bytes each, 1000 bytes combined
- Topic name stays fixed after baseline -- only description + examples change

### Output
When user stops the agent, it reports best-performing definition with metrics.

## Platform Constraints (unchanged)

- Block-intent ceiling: ~40-50% coverage due to vocabulary overlap
- Allow-intent ceiling: ~40-70% coverage
- High-sensitivity domains (explosives, weapons): AIRS built-in safety may override custom definitions
- Topic revision must be included when assigning to profile
- Always reference profiles by NAME, never ID
