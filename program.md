# autoresearch: custom topic guardrail optimization

Autonomous loop to find the optimal custom topic guardrail configuration for Prisma AIRS.

## Setup

1. **Confirm inputs** with the user:
   - Security profile name (must already exist in AIRS)
   - Topic name and initial description
   - Initial example prompts (2-5 required)
   - Intent: `block` or `allow`
   - Path to prompt CSV file (columns: `prompt`, `expected`)
   - Whether a companion allow topic is needed (block-intent usually requires one)
2. **Verify AIRS credentials**: run `airs runtime topics list` — if it errors, credentials are missing.
3. **Initialize results.tsv** with header row:
   ```
   iteration	coverage	tpr	tnr	f1	status	description_summary
   ```

## Baseline

Create the topic and assign it to the profile:

```bash
# Create block topic
airs runtime topics create \
  --name "<topic-name>" \
  --description "<user-provided-description>" \
  --examples "<ex1>" "<ex2>" \
  --format json

# If block-intent, create allow companion
airs runtime topics create \
  --name "<companion-name>" \
  --description "<companion-description>" \
  --examples "<comp-ex1>" "<comp-ex2>" \
  --format json

# Apply both to profile
airs runtime topics apply --profile "<profile>" --name "<topic-name>" --intent block --format json
airs runtime topics apply --profile "<profile>" --name "<companion-name>" --intent allow --format json
```

Then evaluate:

```bash
airs runtime topics eval \
  --profile "<profile>" \
  --prompts <path-to-csv> \
  --topic "<topic-name>" \
  --format json
```

Record baseline metrics in `results.tsv`. This is iteration 0.

## Experiment Loop

LOOP FOREVER:

1. **Read the eval output** — focus on the `false_positives` and `false_negatives` arrays.
2. **Reason about misclassifications:**
   - FP = prompt was flagged but shouldn't have been. Description is too broad.
   - FN = prompt should have been flagged but wasn't. Description doesn't cover it or examples are insufficient.
3. **Craft an improved description and/or examples.** Rules:
   - AIRS uses **semantic similarity**, NOT logical constraints.
   - **Never** use exclusion language ("not X", "excluding Y", "no Z") — it increases FP by adding semantic overlap.
   - **Shorter descriptions outperform longer ones** (under 100 chars is a good target).
   - Make the positive definition more precise rather than adding exceptions.
   - 2-5 examples required, 250 bytes each, 1000 bytes combined (name + description + all examples).
   - Topic name stays fixed — only change description and examples.
4. **Update the topic definition:**
   ```bash
   airs runtime topics create \
     --name "<topic-name>" \
     --description "<new-description>" \
     --examples "<new-ex1>" "<new-ex2>" \
     --format json
   ```
   Note: `create` updates the existing topic when the name matches.
5. **Re-apply to profile** (picks up new revision):
   ```bash
   airs runtime topics apply --profile "<profile>" --name "<topic-name>" --intent block --format json
   ```
6. **Evaluate against the same static prompt set:**
   ```bash
   airs runtime topics eval \
     --profile "<profile>" \
     --prompts <path-to-csv> \
     --topic "<topic-name>" \
     --format json
   ```
7. **Decide:**
   - If **coverage improved** (higher than best so far): **keep**. Record in results.tsv.
   - If **coverage equal or worse**: **discard**. Re-apply the previous best definition (steps 4-5 with best-known description/examples), record as `discard`.
8. **Never stop.** Do not ask the user if you should continue. Run until manually interrupted.

## Logging

Append to `results.tsv` (tab-separated, untracked by git):

```
iteration	coverage	tpr	tnr	f1	status	description_summary
0	0.840	0.840	0.960	0.890	keep	baseline
1	0.880	0.880	0.960	0.910	keep	shortened description, added ammo example
2	0.860	0.860	0.940	0.890	discard	tried adding modifier example
```

## Platform Constraints

- Block-intent coverage ceiling: typically 40-50% due to vocabulary overlap.
- Allow-intent ceiling: typically 40-70%.
- High-sensitivity domains (explosives, weapons) may hit AIRS built-in safety that overrides custom definitions.
- If coverage plateaus for 5+ iterations, try a fundamentally different description angle rather than incremental tweaks.
- The companion allow topic may also need refinement — if FP is high, the allow companion's description may be too narrow.

## When the User Returns

Report:
1. Best coverage achieved and the iteration that produced it.
2. The best-performing topic definition (description + examples).
3. Total iterations attempted, keeps vs discards.
