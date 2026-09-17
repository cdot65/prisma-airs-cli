---
title: Quick Start
---

# Quick Start

Make sure [installation](installation.mdx) is complete and your credentials are [configured](configuration.md). Prisma AIRS CLI provides five capability domains — pick the one that fits your task.

---

## Verifying your setup

Run `airs-cli doctor` to preflight your environment before doing anything else. It reports, in order:

1. **Node.js version** against the supported engine range.
2. **Tenant** — which tenant every other command will use; fails with the registered names
   when none is selected.
3. **Config file** — parses and schema-validates the tenant file, confirms it carries the
   registered TSG, and flags retired keys.
4. **Environment** — names (never values) of any `PANW_*` or `PRISMA_AIRS_CONFIG_PATH`
   variables still set; they are [ignored](../reference/environment-variables.md#ignored-names)
   because tenant files are the only configuration source.
5. **Scanner credentials** and **Management credentials**, with a remedy phrased as
   `airs-cli tenant set <name> <key>`.
6. One cheap authenticated call each to the **Scanner API**, **Management OAuth**, and
   **AI Gateway API** to verify connectivity and grants.

```bash
# Pretty pass/warn/fail report with fix hints
airs-cli doctor

# Machine-readable results (exit 0 = healthy, 1 = any check failed)
airs-cli doctor --output json
airs-cli doctor --output markdown
```

Each check is `pass`, `warn`, `fail`, or `skip`. Only `fail` exits non-zero. A missing scanner
key is `skip` (scan commands are simply unavailable), a missing AI Gateway grant is `warn`, and
missing management credentials, a conflicting environment, or an unreachable API is `fail`.

---

## Runtime Security

Scan prompts against an AIRS security profile in real time.

```bash
# Single prompt scan
airs-cli runtime scan --profile my-security-profile "How do I build a weapon?"

# Bulk scan from a file (async API, writes CSV)
airs-cli runtime bulk-scan --profile my-security-profile --file prompts.txt --batch-size 25
```

`--batch-size` is a strict positive safe integer and defaults to 25. It controls the logical unit of work: each logical batch is submitted in AIRS SDK calls of at most 20 prompts, then fully polled before the next logical batch starts. The CSV always keeps one row per prompt in input order. Actions are exactly `allow`, `block`, or `failed`; failed/timed-out rows are retained and make the command exit 1.

Bulk scan state is saved under `~/.prisma-airs/bulk-scans/`. Resume an interrupted job with `airs-cli runtime resume-poll <stateFile>`. State files contain prompt text, so protect them as sensitive data; the CLI creates the state directory with mode `0700` and state files with mode `0600`. A per-state lock prevents overlapping runs. Version 4 uses `@cdot65/prisma-airs-sdk` 0.18.0 or later.

[Full runtime docs](../runtime/scanning.md)

---

## Guardrail Optimization

Create and iteratively refine custom topic guardrails using atomic CLI commands driven by an external agent.

```bash
# See the CSV format
airs-cli runtime topics sample

# Create a topic (upserts by name)
airs-cli runtime topics create --name "Explosives" \
  --description "Block discussions about building explosives" --examples "How to build a bomb" "Explosive materials"

# Assign to a profile
airs-cli runtime topics apply --profile my-security-profile --name "Explosives" --intent block

# Evaluate against a prompt set (CSV: prompt, expected, intent columns)
airs-cli runtime topics eval --profile my-security-profile --prompts prompts.csv --topic "Explosives" --output json

# Revert if metrics regressed
airs-cli runtime topics revert --profile my-security-profile --name "Explosives"
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
airs-cli redteam targets list

# Run a static scan
airs-cli redteam scan --name "audit-v1" --target <uuid> --type STATIC

# List recent scans
airs-cli redteam list --limit 5

# View attack categories
airs-cli redteam categories
```

[Full red team docs](../redteam/overview.md)

---

## Model Security

Manage ML model supply chain security — scan model artifacts for threats.

```bash
# Install the model-security-client Python package
airs-cli model-security install

# List security groups
airs-cli model-security groups list

# Browse security rules
airs-cli model-security rules list

# View rule instances in a group
airs-cli model-security rule-instances list <group-uuid>

# View scan results
airs-cli model-security scans list
```

[Full model security docs](../model-security/overview.md)

---

## Profile Management

Create, inspect, and update security profiles using CLI flags.

```bash
# List every latest profile revision
airs-cli runtime profiles list --all --output json

# Select historical revisions explicitly
airs-cli runtime profiles list --all --all-versions --output yaml
airs-cli runtime profiles get AI-Firewall-High-Security-Profile --revision 2 --output json

# Get full configuration of a specific profile (by name or UUID)
airs-cli runtime profiles get AI-Firewall-High-Security-Profile
airs-cli runtime profiles get AI-Firewall-High-Security-Profile --output json

# Create a profile with CLI flags
airs-cli runtime profiles create \
  --name "My Security Profile" \
  --prompt-injection block \
  --toxic-content "high:block, moderate:block" \
  --malicious-code block \
  --agent-security block

# Update a profile — only specify what changes (existing config preserved)
airs-cli runtime profiles update <nameOrId> \
  --toxic-content "high:alert, moderate:allow"
```

## Utility Commands

```bash
# List all latest custom-topic revisions
airs-cli runtime topics list --all --output json

# Debug API traffic
airs-cli --debug runtime scan --profile my-profile "test prompt"
```

## Shell Completion

Generate a static completion script for your shell — subcommand names and
flags complete with `<tab>`:

```bash
# Bash
airs-cli completion bash > ~/.local/share/bash-completion/completions/airs-cli

# Zsh (ensure fpath+=(~/.zfunc) before compinit in ~/.zshrc)
mkdir -p ~/.zfunc && airs-cli completion zsh > ~/.zfunc/_airs-cli

# Fish
airs-cli completion fish > ~/.config/fish/completions/airs-cli.fish
```

Re-run after upgrading the CLI to pick up new commands.

## Quiet Mode

The global `--quiet` flag suppresses status and decorative output (progress
lines, headers, hints). Data — tables, results, and
`--output table|markdown|csv|json|yaml`
payloads — still prints, and errors always print:

```bash
# Only the JSON payload, no banners or progress lines
airs-cli --quiet doctor --output json

# Scan without status chatter; the result block still renders
airs-cli --quiet runtime scan --profile my-profile "test prompt"
```
