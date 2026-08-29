---
sidebar_label: list
---

# redteam list

## redteam list

List recent scans

```text
airs redteam list [options]
```

### Options

| Flag | Required | Default | Description |
|------|:--------:|---------|-------------|
| `--status <status>` | No | — | Filter by status (QUEUED, RUNNING, COMPLETED, FAILED, ABORTED) |
| `--type <type>` | No | — | Filter by job type (STATIC, DYNAMIC, CUSTOM) |
| `--target <uuid>` | No | — | Filter by target UUID |
| `--limit <n>` | No | `10` | Max results |
| `--offset <n>` | No | `0` | Starting offset |
| `--all` | No | — | Walk every page |
| `--max <n>` | No | `10000` | Safety cap for `--all`; `0` removes the cap |
| `--output <format>` | No | Resolved | Output format: pretty, table, markdown, csv, json, yaml |

### Examples

*List recent scans*

```bash
airs redteam list
```

```text
Recent Scans:

304becf3-7090-413a-aa41-2cd327b7f0c5
  Pokemon guardrail validation  COMPLETED  CUSTOM  score: 0.43
  2026-03-08T11:11:21.371253Z

06711c07-69de-4a79-b61c-4c03d1175694
  E2E Custom Scan - Explosives Topic v2  COMPLETED  CUSTOM  score: 12.5
  2026-03-08T10:37:56.654621Z

19f5dd0e-e06f-45d4-9ebe-b45ca3f20e42
  litellm.cdot.io - no guardrails - 004  ABORTED  STATIC
  2026-03-06T01:42:28.477755Z
```
