---
title: Guardrail to Red Team
---

# Guardrail Generation to Red Team Scan

This workflow walks through a complete end-to-end cycle: generate a custom topic guardrail, export the test cases as a prompt set, then red-team your AI application using those prompts.

## Prerequisites

- Prisma AIRS CLI installed and configured ([Installation](../getting-started/installation.md))
- AIRS credentials set ([Configuration](../getting-started/configuration.md))
- A security profile in Prisma AIRS
- A red team target configured in AI Runtime Security

## Step 1: Generate a Guardrail + Prompt Set

Use `airs runtime topics generate` with `--create-prompt-set` to build a topic guardrail **and** automatically export the best iteration's test cases as a custom prompt set in AI Red Team.

```bash
airs runtime topics generate \
  --profile "Custom Topics Test" \
  --topic "Pokemon discussions" \
  --intent block \
  --max-iterations 3 \
  --target-coverage 90 \
  --create-prompt-set \
  --prompt-set-name "pokemon-guardrail-tests"
```

When the loop completes, Prisma AIRS CLI:

1. Deploys the refined topic guardrail to your AIRS profile
2. Creates a custom prompt set named `pokemon-guardrail-tests` in AI Red Team
3. Prints the prompt set name and prompt count

## Step 2: Find Your Prompt Set UUID

```bash
airs redteam prompt-sets list
```

Copy the UUID for your prompt set from the output.

## Step 3: Find Your Red Team Target

```bash
airs redteam targets list
```

Copy the target UUID for the next step.

## Step 4: Launch a Custom Red Team Scan

Run a CUSTOM scan using the prompt set UUID against your target. Add `--no-wait` to submit and return immediately:

```bash
airs redteam scan \
  --target <target-uuid> \
  --name "Pokemon guardrail validation" \
  --type CUSTOM \
  --prompt-sets <prompt-set-uuid> \
  --no-wait
```

## Step 5: Check Scan Status

```bash
airs redteam status <jobId>
```

Re-run periodically until the status changes to `COMPLETED`.

## Step 6: View the Report

```bash
airs redteam report <jobId>
```

Add `--attacks` to see individual prompt outcomes:

```bash
airs redteam report <jobId> --attacks
```

Each prompt shows THREAT/SAFE status, ASR across attempts, and the goal from test case generation.

## Step 7: Iterate

If the ASR is too high (meaning the target is vulnerable), you can:

1. **Add guardrails** — deploy the topic guardrail to the target's security profile
2. **Re-scan** — run the same prompt set again to validate the guardrail is effective
3. **Re-run generation** with more iterations or a higher coverage target
4. **Resume a previous run** with `airs runtime topics resume <runId>` to continue refining
5. **Abort a running scan** if needed: `airs redteam abort <jobId>`

## Complete Script

```bash
#!/usr/bin/env bash
set -euo pipefail

PROFILE="Custom Topics Test"
TOPIC="Pokemon discussions"
TARGET_UUID="<target-uuid>"
PROMPT_SET_NAME="pokemon-guardrail-tests"

# 1. Generate guardrail + export prompt set
airs runtime topics generate \
  --profile "$PROFILE" \
  --topic "$TOPIC" \
  --intent block \
  --max-iterations 3 \
  --target-coverage 90 \
  --create-prompt-set \
  --prompt-set-name "$PROMPT_SET_NAME"

# 2. Find the prompt set UUID
airs redteam prompt-sets list
PROMPT_SET_UUID="<uuid-from-output>"

# 3. Launch red team scan (async)
airs redteam scan \
  --target "$TARGET_UUID" \
  --name "Validate: $TOPIC" \
  --type CUSTOM \
  --prompt-sets "$PROMPT_SET_UUID" \
  --no-wait

# 4. Check status
JOB_ID="<job-id-from-step-3>"
airs redteam status "$JOB_ID"

# 5. View report with per-prompt details
airs redteam report "$JOB_ID" --attacks
```

!!! note "Replace placeholder values"
    Replace UUIDs and job IDs with actual values from your run.
