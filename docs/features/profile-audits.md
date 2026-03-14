---
title: Profile Audits
---

# Profile Audits

Evaluate all topics in a Prisma AIRS security profile at once. The audit generates test prompts per topic, scans them, computes per-topic and composite metrics, and detects cross-topic conflicts.

## CLI Usage

```bash
# Terminal output (default)
airs audit my-security-profile

# JSON report
airs audit my-security-profile --format json

# HTML report
airs audit my-security-profile --format html --output audit-report.html
```

### Options

| Flag | Default | Description |
|------|---------|-------------|
| `--max-tests-per-topic <n>` | `20` | Max test cases generated per topic |
| `--format <fmt>` | `terminal` | Output format: `terminal`, `json`, `html` |
| `--output <path>` | `<profile>-audit.html` | Output file path (html format only) |
| `--provider <name>` | `claude-api` | LLM provider for test generation |
| `--model <name>` | per-provider | Override default model |

## How It Works

1. **Load topics** — reads all topics from the profile's `topic-guardrails` policy
2. **Generate tests** — LLM produces test prompts per topic, tagged with `targetTopic`
3. **Batch scan** — all prompts scanned against the profile
4. **Per-topic evaluation** — TPR, TNR, coverage, F1 computed for each topic
5. **Composite metrics** — aggregate metrics across all topics
6. **Conflict detection** — finds prompts that fail as FN for one topic and FP for another

## Conflict Detection

A conflict occurs when the same prompt is a false negative for topic A and a false positive for topic B. This indicates the topics have overlapping or contradictory definitions.

Conflicts are reported with the specific prompt text and the two topics involved, helping you refine definitions to eliminate ambiguity.

## Output Formats

- **Terminal** — per-topic metrics table, composite metrics, conflict list
- **JSON** — structured `AuditResult` object for programmatic use
- **HTML** — self-contained report with embedded CSS
