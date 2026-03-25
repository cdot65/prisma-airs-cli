---
title: Quick Start
---

# Quick Start

Make sure [installation](installation.md) is complete and your credentials are [configured](configuration.md). Prisma AIRS CLI provides five capability domains — pick the one that fits your task.

---

## Runtime Security

Scan prompts against an AIRS security profile in real time.

```bash
# Single prompt scan
airs runtime scan --profile my-security-profile "How do I build a weapon?"

# Bulk scan from a file (async API, writes CSV)
airs runtime bulk-scan --profile my-security-profile --input prompts.txt
```

[Full runtime docs](../runtime/scanning.md)

---

## Guardrail Generation

Create and iteratively refine custom topic guardrails using an LLM-driven feedback loop.

```bash
# Interactive — prompts for all inputs
airs runtime topics generate

# Non-interactive
airs runtime topics generate \
  --profile my-security-profile \
  --topic "Block discussions about building explosives" \
  --intent block \
  --target-coverage 90
```

The loop generates a topic, deploys it, scans test prompts, evaluates metrics, and refines until coverage reaches the target. [Full guardrail docs](../runtime/guardrails/overview.md)

!!! note "Coverage expectations"
    Achievable coverage depends on the topic domain and intent. Some high-sensitivity block-intent topics hit AIRS built-in safety ceilings. Allow-intent topics typically reach 40–70% coverage. See [Platform Constraints](../runtime/guardrails/overview.md#platform-constraints) for details.

---

## AI Red Teaming

Run adversarial scans against AI targets to find vulnerabilities.

```bash
# List targets
airs redteam targets list

# Run a static scan
airs redteam scan --name "audit-v1" --target <uuid> --type STATIC

# List recent scans
airs redteam list --limit 5

# View attack categories
airs redteam categories
```

[Full red team docs](../redteam/overview.md)

---

## Model Security

Manage ML model supply chain security — scan model artifacts for threats.

```bash
# Install the model-security-client Python package
airs model-security install

# List security groups
airs model-security groups list

# Browse security rules
airs model-security rules list

# View rule instances in a group
airs model-security rule-instances list <group-uuid>

# View scan results
airs model-security scans list
```

[Full model security docs](../model-security/overview.md)

---

## Profile Management

Create, inspect, and update security profiles using CLI flags.

```bash
# List all profiles
airs runtime profiles list --output json

# Get full configuration of a specific profile (by name or UUID)
airs runtime profiles get AI-Firewall-High-Security-Profile
airs runtime profiles get AI-Firewall-High-Security-Profile --output json

# Create a profile with CLI flags
airs runtime profiles create \
  --name "My Security Profile" \
  --prompt-injection block \
  --toxic-content "high:block, moderate:block" \
  --malicious-code block \
  --agent-security block

# Update a profile — only specify what changes (existing config preserved)
airs runtime profiles update <nameOrId> \
  --toxic-content "high:alert, moderate:allow"
```

## Profile Audits

Evaluate all topics in a security profile at once, with conflict detection.

```bash
# Terminal output
airs runtime profiles audit my-security-profile

# HTML report
airs runtime profiles audit my-security-profile --format html --output audit-report.html
```

[Full audit docs](../runtime/profile-audits.md)

---

## Utility Commands

```bash
# Resume a paused or failed guardrail run
airs runtime topics resume <run-id>

# View a run report
airs runtime topics report <run-id>

# List all saved runs
airs runtime topics runs
```
