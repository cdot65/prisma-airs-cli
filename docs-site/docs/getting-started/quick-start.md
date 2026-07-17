---
title: Quick Start
---

# Quick Start

Make sure [installation](installation.mdx) is complete and your credentials are [configured](configuration.md). Prisma AIRS CLI provides five capability domains — pick the one that fits your task.

---

## Verifying your setup

Run `airs doctor` to preflight your environment before doing anything else. It checks your Node.js version, config file, which credentials are set (and from where), and makes one cheap authenticated call to each API to verify connectivity.

```bash
# Pretty pass/warn/fail report with fix hints
airs doctor

# Machine-readable results (exit 0 = healthy, 1 = any check failed)
airs doctor --output json
```

Warnings (e.g. no config file when using env vars only) do not fail the command — only hard failures like missing credentials or unreachable APIs exit non-zero.

---

## Runtime Security

Scan prompts against an AIRS security profile in real time.

```bash
# Single prompt scan
airs runtime scan --profile my-security-profile "How do I build a weapon?"

# Bulk scan from a file (async API, writes CSV)
airs runtime bulk-scan --profile my-security-profile --file prompts.txt --batch-size 25
```

`--batch-size` is a strict positive safe integer and defaults to 25. It controls the logical unit of work: each logical batch is submitted in AIRS SDK calls of at most 20 prompts, then fully polled before the next logical batch starts. The CSV always keeps one row per prompt in input order. Actions are exactly `allow`, `block`, or `failed`; failed/timed-out rows are retained and make the command exit 1.

Bulk scan state is saved under `~/.prisma-airs/bulk-scans/`. Resume an interrupted job with `airs runtime resume-poll <stateFile>`. State files contain prompt text, so protect them as sensitive data; the CLI creates the state directory with mode `0700` and state files with mode `0600`. A per-state lock prevents overlapping runs. Bulk scanning requires `@cdot65/prisma-airs-sdk` 0.13.2 or later.

[Full runtime docs](../runtime/scanning.md)

---

## Guardrail Optimization

Create and iteratively refine custom topic guardrails using atomic CLI commands driven by an external agent.

```bash
# See the CSV format
airs runtime topics sample

# Create a topic (upserts by name)
airs runtime topics create --name "Explosives" \
  --description "Block discussions about building explosives" --examples "How to build a bomb" "Explosive materials"

# Assign to a profile
airs runtime topics apply --profile my-security-profile --name "Explosives" --intent block

# Evaluate against a prompt set (CSV: prompt, expected, intent columns)
airs runtime topics eval --profile my-security-profile --prompts prompts.csv --topic "Explosives" --output json

# Revert if metrics regressed
airs runtime topics revert --profile my-security-profile --name "Explosives"
```

The full autonomous optimization loop is defined in [`AGENTS.md`](https://github.com/cdot65/prisma-airs-cli/blob/main/AGENTS.md) for use with AI agents (Claude Code, Codex, Copilot, etc.). [Full guardrail docs](../runtime/guardrails/overview.md)

:::note[Coverage expectations]
Achievable coverage depends on the topic domain and intent. Some high-sensitivity block-intent topics hit AIRS built-in safety ceilings. Allow-intent topics typically reach 40-70% coverage. See [Platform Constraints](../runtime/guardrails/overview.md#platform-constraints) for details.
:::

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

## Utility Commands

```bash
# List all custom topics
airs runtime topics list --output json

# Debug API traffic
airs --debug runtime scan --profile my-profile "test prompt"
```

## Shell Completion

Generate a static completion script for your shell — subcommand names and
flags complete with `<tab>`:

```bash
# Bash
airs completion bash > ~/.local/share/bash-completion/completions/airs

# Zsh (ensure fpath+=(~/.zfunc) before compinit in ~/.zshrc)
mkdir -p ~/.zfunc && airs completion zsh > ~/.zfunc/_airs

# Fish
airs completion fish > ~/.config/fish/completions/airs.fish
```

Re-run after upgrading the CLI to pick up new commands.

## Quiet Mode

The global `--quiet` flag suppresses status and decorative output (progress
lines, headers, hints). Data — tables, results, and `--output json|yaml|csv`
payloads — still prints, and errors always print:

```bash
# Only the JSON payload, no banners or progress lines
airs --quiet doctor --output json

# Scan without status chatter; the result block still renders
airs --quiet runtime scan --profile my-profile "test prompt"
```
