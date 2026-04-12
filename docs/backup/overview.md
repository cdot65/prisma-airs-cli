---
title: Backup & Restore
---

# Backup & Restore

Export and import AIRS configuration to local JSON or YAML files. Currently supports red team targets, with future support planned for profiles, topics, and prompt sets.

## Prerequisites

- Prisma AIRS CLI installed and configured ([Installation](../getting-started/installation.md))
- AIRS management credentials set (`PANW_MGMT_CLIENT_ID`, `PANW_MGMT_CLIENT_SECRET`, `PANW_MGMT_TSG_ID`)

---

## Backup Targets

### Bulk Backup (All Targets)

Export every red team target to a directory, one file per target:

```bash
airs redteam targets backup
```

Default output directory is `./airs-backup/targets/`. Each file is named after the target (sanitized to filesystem-safe characters).

**Example output:**

```
  Prisma AIRS — Backup & Restore

  Backed up 10 target(s) to /Users/you/project/airs-backup/targets:

    ✓ truffles - dev - chat → truffles-dev-chat.json
    ✓ truffles - dev - langgraph agent → truffles-dev-langgraph-agent.json
    ✓ e2e-tf-dbg3-target → e2e-tf-dbg3-target.json
    ✓ e2e-tf-dbg2-target → e2e-tf-dbg2-target.json
    ✓ e2e-tf-ser1-target → e2e-tf-ser1-target.json
    ✓ e2e-tf-19f9-target → e2e-tf-19f9-target.json
    ✓ e2e-tf-616f-target → e2e-tf-616f-target.json
    ✓ e2e-tf-target → e2e-tf-target.json
    ...
```

### Single Target Backup

Export a specific target by name:

```bash
airs redteam targets backup --name "truffles - dev - langgraph agent"
```

### Options

| Flag | Default | Description |
|------|---------|-------------|
| `--output-dir <path>` | `./airs-backup/targets/` | Directory to write backup files |
| `--format <format>` | `json` | Output format: `json` or `yaml` |
| `--name <targetName>` | _(all targets)_ | Backup a single target by name |

### YAML Format

```bash
airs redteam targets backup --format yaml --output-dir ./my-backups
```

---

## Restore Targets

### From a Single File

```bash
airs redteam targets restore --file ./airs-backup/targets/my-target.json
```

### From a Directory

Restore all backup files in a directory:

```bash
airs redteam targets restore --input-dir ./airs-backup/targets/
```

### Handling Collisions

By default, targets with matching names are **skipped** with a warning. Use `--overwrite` to update existing targets:

```bash
airs redteam targets restore --input-dir ./airs-backup/targets/ --overwrite
```

**Example output:**

```
  Restore results:

    ✓ truffles - dev - langgraph agent — created
    ✓ e2e-tf-dbg3-target — updated
    ○ e2e-tf-dbg2-target — skipped

  Total: 1 created, 1 updated, 1 skipped
```

### Validating Connections

Test each target's connection before saving:

```bash
airs redteam targets restore --file ./my-target.json --validate
```

### Options

| Flag | Default | Description |
|------|---------|-------------|
| `--input-dir <path>` | _(required if no --file)_ | Directory containing backup files |
| `--file <path>` | _(required if no --input-dir)_ | Single backup file to restore |
| `--overwrite` | `false` | Update existing targets with same name |
| `--validate` | `false` | Test connection before saving |

!!! warning "Credentials in backup files"
    Backup files contain connection credentials (API keys, tokens, auth configs) in plaintext. Store backup files securely and avoid committing them to version control.

---

## Backup File Format

Each backup file wraps the target configuration in a versioned envelope:

```json
{
  "version": "1",
  "resourceType": "redteam-target",
  "exportedAt": "2026-04-12T02:51:20.624Z",
  "data": {
    "name": "my-chat-target",
    "target_type": "APPLICATION",
    "connection_params": {
      "api_endpoint": "https://example.com/api/v1/chat/completions",
      "request_headers": { "Content-Type": "application/json", "Authorization": "Bearer ..." },
      "request_json": { "messages": [{"role": "user", "content": "{INPUT}"}], "stream": false },
      "response_json": { "choices": [{"index": 0, "message": {"role": "assistant", "content": "{RESPONSE}"}}] },
      "response_key": "content"
    },
    "target_background": { "industry": "tech", "use_case": "customer support" },
    "additional_context": { "system_prompt": "You are a helpful assistant." },
    "target_metadata": { "multi_turn": false, "rate_limit": 10 }
  }
}
```

- **`version`** — envelope format version (currently `"1"`)
- **`resourceType`** — resource discriminator (`"redteam-target"`)
- **`exportedAt`** — ISO 8601 timestamp of the backup
- **`data`** — the target configuration in create-request format (server-assigned fields like `uuid`, `status`, and `active` are stripped)

Both JSON and YAML formats are supported. Format is auto-detected from file extension on restore.

---

## Use Cases

### Disaster Recovery

Back up all targets before making changes:

```bash
# Before changes
airs redteam targets backup --output-dir ./pre-change-backup/

# After testing, if something went wrong
airs redteam targets restore --input-dir ./pre-change-backup/ --overwrite
```

### Environment Migration

Move targets between AIRS tenants:

```bash
# Export from source tenant
PANW_MGMT_TSG_ID=source-tsg airs redteam targets backup

# Import to destination tenant
PANW_MGMT_TSG_ID=dest-tsg airs redteam targets restore --input-dir ./airs-backup/targets/
```

### Version Control

Store target configurations in git for audit trails:

```bash
airs redteam targets backup --output-dir ./infra/airs-targets/ --format yaml
git add infra/airs-targets/
git commit -m "snapshot: AIRS red team targets"
```

!!! tip
    YAML format is more readable in diffs and code reviews. Use `--format yaml` for version-controlled backups.
