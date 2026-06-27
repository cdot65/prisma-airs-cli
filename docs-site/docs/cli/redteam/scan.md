---
sidebar_label: scan
---

# redteam scan

## redteam scan

Execute a red team scan against a target

```text
airs redteam scan [options]
```

### Options

| Flag | Required | Default | Description |
|------|:--------:|---------|-------------|
| `--target <uuid>` | Yes | — | Target UUID |
| `--name <name>` | Yes | — | Scan name |
| `--type <type>` | No | `STATIC` | Job type: STATIC, DYNAMIC, or CUSTOM |
| `--categories <json>` | No | — | Category filter JSON (STATIC scans) |
| `--prompt-sets <uuids>` | No | — | Comma-separated prompt set UUIDs (CUSTOM scans) |
| `--goals <file>` | No | — | JSON file or inline JSON array of attack goals (DYNAMIC scans) |
| `--depth <number>` | No | `10` | Max conversation turns per goal (DYNAMIC scans) |
| `--breadth <number>` | No | `6` | Parallel agents per goal (DYNAMIC scans) |
| `--no-wait` | No | — | Submit scan without waiting for completion |

### Examples

*Static scan with all categories*

```bash
airs redteam scan --target <uuid> --name "Full Scan"
```

*Custom scan with prompt sets*

```bash
airs redteam scan --target <uuid> --name "Topic Validation" --type CUSTOM --prompt-sets <uuid1>,<uuid2>
```

*Dynamic scan with goals file*

```bash
airs redteam scan --target <uuid> --name "Agent Scan" --type DYNAMIC --goals goals.json --depth 10 --breadth 6
```
