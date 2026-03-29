# Guardrail Optimization Protocol

Autonomous loop to find the optimal custom topic guardrail configuration for Prisma AIRS. This protocol is designed to be executed by any AI coding agent (Claude Code, Codex, Gemini CLI, Copilot, Pi, etc.).

You are the intelligence. The CLI is the instrument. Read eval output, reason about misclassifications, craft better definitions, decide keep/discard.

## Prerequisites

- `airs` CLI installed and on PATH
- AIRS credentials configured (env vars or `~/.prisma-airs/config.json`):
  - `PANW_AI_SEC_API_KEY` — for scanning
  - `PANW_MGMT_CLIENT_ID`, `PANW_MGMT_CLIENT_SECRET`, `PANW_MGMT_TSG_ID` — for topic/profile management
- A security profile already created in AIRS
- A prompt CSV file (see CSV Format below)

## Setup

1. **Confirm inputs** with the user:
   - Security profile name (must already exist in AIRS)
   - Topic name and initial description
   - Initial example prompts (2-5 required, each max 250 bytes)
   - Intent: `block` or `allow`
   - Path to prompt CSV file
   - Whether companion topics may be needed (see Companion Topics below)

2. **Verify AIRS credentials:**
   ```bash
   airs runtime topics list --output json
   ```
   If this errors, credentials are missing. Stop and tell the user.

3. **Read current topic state** (if topic already exists):
   ```bash
   airs runtime topics get "<topic-name>" --output json
   ```
   Save the full response (description + examples). This is your revert target. If the topic doesn't exist yet, skip this step.

4. **Initialize results.tsv:**
   ```
   iteration	coverage	tpr	tnr	f1	status	description	examples
   ```

## CSV Format

The eval command reads a CSV with three required columns:

| Column | Meaning |
|--------|---------|
| `prompt` | The prompt text to scan |
| `expected` | Does this prompt belong to the topic category? `true` or `false` |
| `intent` | `block` or `allow` — how the topic is applied to the profile |

The `expected` column is intuitive — mark `true` if the prompt is about the topic, `false` if it isn't. The `intent` column tells the eval command how to interpret that:

- **block intent:** `expected=true` → should trigger a violation (block it). `expected=false` → should NOT trigger.
- **allow intent:** `expected=true` → should NOT trigger (it's allowed through). `expected=false` → SHOULD trigger (it's outside allowed bounds).

Run `airs runtime topics sample` to see an example CSV.

**Tips for good eval sets:**
- Aim for 50/50 balance between expected=true and expected=false
- Include hard negatives: prompts semantically close to the topic but NOT about it
- All rows must have the same intent value
- 50-100 total prompts is a good starting size

## Baseline (Iteration 0)

Create the topic and establish baseline metrics:

```bash
# Create the topic (upserts by name if it already exists)
airs runtime topics create \
  --name "<topic-name>" \
  --description "<description>" \
  --examples "<ex1>" "<ex2>" \
  --format json

# Apply to the security profile
airs runtime topics apply \
  --profile "<profile>" \
  --name "<topic-name>" \
  --intent <block|allow> \
  --format json

# Evaluate
airs runtime topics eval \
  --profile "<profile>" \
  --prompts <path-to-csv> \
  --topic "<topic-name>" \
  --format json
```

Record iteration 0 in results.tsv with the full definition (description + examples). This is your "best known" state.

## Experiment Loop

LOOP:

1. **Read the eval JSON output.** Focus on `false_positives` and `false_negatives` arrays.

2. **Reason about misclassifications:**
   - **FP** (false positive) = prompt triggered but shouldn't have. The topic definition is too broad — it's matching content outside the intended category.
   - **FN** (false negative) = prompt should have triggered but didn't. The definition doesn't cover this content — it's too narrow or the examples don't represent this case.

3. **Craft an improved description and/or examples.** Follow these rules:
   - AIRS uses **semantic similarity matching**, NOT logical rules or keyword matching.
   - **Never use exclusion language** ("not X", "excluding Y", "no Z"). This adds semantic overlap with the excluded concept and INCREASES false positives.
   - **Shorter descriptions outperform longer ones.** Target under 100 characters.
   - **Examples significantly change the semantic profile.** Small example changes can cause dramatic metric swings. Change one thing at a time.
   - Make the positive definition more precise rather than adding exceptions.
   - Constraints: 2-5 examples, each max 250 bytes, name + description + all examples combined max 1000 bytes.
   - **Topic name stays fixed.** Only change description and examples.

4. **Read current topic state before modifying:**
   ```bash
   airs runtime topics get "<topic-name>" --output json
   ```
   Confirm this matches your expected "current" state. Save it — you need it for revert if this iteration fails.

5. **Update the topic:**
   ```bash
   airs runtime topics create \
     --name "<topic-name>" \
     --description "<new-description>" \
     --examples "<new-ex1>" "<new-ex2>" \
     --format json
   ```

6. **Re-apply to profile** (picks up the new revision):
   ```bash
   airs runtime topics apply \
     --profile "<profile>" \
     --name "<topic-name>" \
     --intent <block|allow> \
     --format json
   ```

7. **Evaluate:**
   ```bash
   airs runtime topics eval \
     --profile "<profile>" \
     --prompts <path-to-csv> \
     --topic "<topic-name>" \
     --format json
   ```

8. **Decide — compare coverage to best-known:**
   - **Coverage improved** (strictly greater than best): **KEEP.** Update best-known state (description + examples + metrics). Log as `keep`.
   - **Coverage equal or worse**: **DISCARD.** Revert to best-known state (step 9). Log as `discard`.

9. **Revert procedure** (on discard):
   ```bash
   # Re-create with the saved best-known definition
   airs runtime topics create \
     --name "<topic-name>" \
     --description "<best-description>" \
     --examples "<best-ex1>" "<best-ex2>" \
     --format json

   # Re-apply
   airs runtime topics apply \
     --profile "<profile>" \
     --name "<topic-name>" \
     --intent <block|allow> \
     --format json
   ```
   **Critical:** Revert means restoring the EXACT best-known description AND examples. Not just the description.

10. **Check for plateau:** If the last 5 iterations were all discards, **STOP** and report to the user (see When to Stop below).

11. **Otherwise, go to step 1.**

## Logging

Append each iteration to `results.tsv` (tab-separated, untracked by git):

```
iteration	coverage	tpr	tnr	f1	status	description	examples
0	0.808	0.808	0.980	0.884	keep	Discussions about the MLB team from the city of Houston, Texas	Houston Astros roster|Astros game score
1	0.635	0.635	0.980	0.767	discard	Houston Astros MLB baseball team	Astros World Series championship|Jose Altuve and the Astros
```

Examples are pipe-delimited within the examples column.

## When to Stop

**After 5 consecutive discards**, stop the loop and report to the user:

1. Best coverage achieved and which iteration produced it.
2. The best-performing topic definition (description + examples).
3. Total iterations attempted — keeps vs discards.
4. The remaining FP and FN patterns.
5. Recommendations:
   - If FN are from completely unrelated content (other teams, other topics), suggest **companion topics** to extend coverage.
   - If FP are from semantically adjacent content, suggest refining the description angle.
   - If coverage has plateaued, suggest the user review whether the CSV expected values are correct.

Let the user decide whether to continue with a new angle, add companion topics, or accept the current best.

## Companion Topics

A single topic has a coverage ceiling — it can only match content semantically similar to its definition. If false negatives are about completely unrelated content (e.g., "Houston Astros" topic can't catch "Chicago Cubs" prompts), that content is outside the topic's semantic reach.

**When to add companions:**
- FN are consistently about a different domain that the primary topic can't cover
- Coverage has plateaued despite description/example changes
- The user agrees to expand the topic constellation

**How to manage companions:**
- Create the companion: `airs runtime topics create --name "<companion>" ...`
- Apply it: `airs runtime topics apply --profile "<profile>" --name "<companion>" --intent <intent>`
- Re-eval: the same prompt CSV now evaluates against the full profile (all topics)
- Track companion definitions in results.tsv alongside the primary
- The optimization loop can refine companions the same way it refines the primary

## Platform Notes

- AIRS may need a few seconds to propagate topic changes before scans reflect them. If you see inconsistent results immediately after an update, wait 10-15 seconds and re-eval.
- The `create` command upserts by name — if a topic with the same name exists, it updates instead of creating a duplicate.
- The `apply` command is additive — it preserves other topics already assigned to the profile.
- Use `--format json` on all commands so you can parse output programmatically.
