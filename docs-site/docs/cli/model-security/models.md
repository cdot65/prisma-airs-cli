---
sidebar_label: models
---

# model-security models

Browse the scanned **model catalog** (read-only): models, their versions, and the
files within each version. Credentials are shared with the other Model Security
commands (`PANW_MGMT_*`).

### model-security models list

List models in the catalog.

```text
airs model-security models list [options]
```

#### Options

| Flag | Required | Default | Description |
|------|:--------:|---------|-------------|
| `--search <text>` | No | — | Filter by search text |
| `--search-query <text>` | No | — | Filter by model UUID or name |
| `--sort-field <field>` | No | — | Sort field: `created_at`, `updated_at` |
| `--sort-order <order>` | No | — | Sort order: `asc`, `desc` |
| `--limit <n>` | No | — | Max results |
| `--offset <n>` | No | — | Starting offset |
| `--output <format>` | No | `pretty` | Output format: pretty, table, csv, json, yaml |

#### Examples

```bash
airs model-security models list
airs model-security models list --search-query llama --sort-field updated_at --sort-order desc
```

### model-security models get

Get a single model by UUID.

```text
airs model-security models get <uuid> [options]
```

| Flag | Required | Default | Description |
|------|:--------:|---------|-------------|
| `--output <format>` | No | `pretty` | Output format: pretty, json, yaml |

### model-security models versions

List the versions of a model.

```text
airs model-security models versions <modelUuid> [options]
```

| Flag | Required | Default | Description |
|------|:--------:|---------|-------------|
| `--sort-order <order>` | No | — | Sort order: `asc`, `desc` |
| `--limit <n>` | No | — | Max results |
| `--offset <n>` | No | — | Starting offset |
| `--output <format>` | No | `pretty` | Output format: pretty, table, csv, json, yaml |

### model-security models version

Get a single model version by UUID (includes last-eval summary).

```text
airs model-security models version <uuid> [options]
```

| Flag | Required | Default | Description |
|------|:--------:|---------|-------------|
| `--output <format>` | No | `pretty` | Output format: pretty, json, yaml |

### model-security models files

List the files scanned in a model version.

```text
airs model-security models files <modelVersionUuid> [options]
```

| Flag | Required | Default | Description |
|------|:--------:|---------|-------------|
| `--limit <n>` | No | — | Max results |
| `--offset <n>` | No | — | Starting offset |
| `--output <format>` | No | `pretty` | Output format: pretty, table, csv, json, yaml |
