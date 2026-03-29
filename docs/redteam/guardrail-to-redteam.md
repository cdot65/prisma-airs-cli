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

## Step 1: Create a Guardrail

Create a custom topic guardrail and optimize it using the atomic CLI commands (or let an AI agent follow the protocol in `program.md`):

```bash
# Create and apply the topic
airs runtime topics create --name "Pokemon" \
  --description "Pokemon discussions" --examples "Pikachu evolution" "Pokemon battle strategy"
airs runtime topics apply --profile "Custom Topics Test" --name "Pokemon" --intent block

# Evaluate against a prompt set (CSV: prompt, expected, intent columns)
airs runtime topics eval --profile "Custom Topics Test" --prompts pokemon-prompts.csv --topic "Pokemon" --format json
```

Once you have an optimized guardrail, export the eval prompts as a red team prompt set:

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
3. **Re-evaluate** with `airs runtime topics eval` using updated prompts and iterate
4. **Revert a topic** with `airs runtime topics revert` to roll back a bad change
5. **Abort a running scan** if needed: `airs redteam abort <jobId>`

## Complete Script

```bash
#!/usr/bin/env bash
set -euo pipefail

PROFILE="Custom Topics Test"
TOPIC="Pokemon"
TARGET_UUID="<target-uuid>"

# 1. Create and apply topic guardrail
airs runtime topics create --name "$TOPIC" \
  --description "Pokemon discussions" --examples "Pikachu evolution" "Pokemon battle strategy"
airs runtime topics apply --profile "$PROFILE" --name "$TOPIC" --intent block

# 2. Evaluate (iterate as needed)
airs runtime topics eval --profile "$PROFILE" --prompts pokemon-prompts.csv --topic "$TOPIC" --format json

# 3. Find the prompt set UUID
airs redteam prompt-sets list
PROMPT_SET_UUID="<uuid-from-output>"

# 4. Launch red team scan (async)
airs redteam scan \
  --target "$TARGET_UUID" \
  --name "Validate: $TOPIC" \
  --type CUSTOM \
  --prompt-sets "$PROMPT_SET_UUID" \
  --no-wait

# 5. Check status
JOB_ID="<job-id-from-step-4>"
airs redteam status "$JOB_ID"

# 6. View report with per-prompt details
airs redteam report "$JOB_ID" --attacks
```

!!! note "Replace placeholder values"
    Replace UUIDs and job IDs with actual values from your run.
