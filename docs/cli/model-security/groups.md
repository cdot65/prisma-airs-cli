# model-security groups

### model-security groups list

List security groups

```text
airs model-security groups list [options]
```

#### Options

| Flag | Required | Default | Description |
|------|:--------:|---------|-------------|
| `--source-types <types>` | No | — | Filter by source types (comma-separated) |
| `--search <query>` | No | — | Search by name or UUID |
| `--sort-field <field>` | No | — | Sort field (created_at, updated_at) |
| `--sort-dir <dir>` | No | — | Sort direction (asc, desc) |
| `--enabled-rules <uuids>` | No | — | Filter by enabled rule UUIDs (comma-separated) |
| `--limit <n>` | No | `20` | Max results |
| `--output <format>` | No | `pretty` | Output format: pretty, table, csv, json, yaml |

#### Examples

*List security groups*

```bash
airs model-security groups list
```

```text
Security Groups:

bb1d038a-0506-4b07-8f16-a723b8c1a1c7
  Default GCS  ACTIVE  source: GCS
020d546d-3920-4ef3-9183-00f37f33f566
  Default LOCAL  ACTIVE  source: LOCAL
6a1e67e1-00cc-45dc-9395-3a9e3dbf50f9
  Default S3  ACTIVE  source: S3
fd1a4209-32d0-4a1a-bd40-cde35104dc39
  Default AZURE  ACTIVE  source: AZURE
4c22aef7-2ab7-40ba-b3f1-cd9e9aa1768e
  Default HUGGING_FACE  ACTIVE  source: HUGGING_FACE
```

---

### model-security groups get

Get security group details

```text
airs model-security groups get [options] <uuid>
```

#### Arguments

- `uuid` (required) —

#### Examples

*Pretty output (default; no --output flag on this command)*

```bash
airs model-security groups get 00000000-0000-0000-0000-000000000001
```

```text
Prisma AIRS — Model Security
ML model supply chain security


Security Group Detail:

  UUID:        00000000-0000-0000-0000-000000000001
  Name:        Default HUGGING_FACE
  Description: Auto-created default security group for HUGGING_FACE models
  Source Type: HUGGING_FACE
  State:       ACTIVE
  Created:     2025-11-24T15:44:39.596957Z
  Updated:     2025-11-24T15:44:48.852089Z
```

---

### model-security groups create

Create a security group

```text
airs model-security groups create [options]
```

#### Options

| Flag | Required | Default | Description |
|------|:--------:|---------|-------------|
| `--config <path>` | Yes | — | JSON file with group configuration |

#### Examples

!!! warning "Example needed"
    No curated input/output example for this command yet.

---

### model-security groups update

Update a security group

```text
airs model-security groups update [options] <uuid>
```

#### Arguments

- `uuid` (required) —

#### Options

| Flag | Required | Default | Description |
|------|:--------:|---------|-------------|
| `--name <name>` | No | — | New name |
| `--description <desc>` | No | — | New description |

#### Examples

!!! warning "Example needed"
    No curated input/output example for this command yet.

---

### model-security groups delete

Delete a security group

```text
airs model-security groups delete [options] <uuid>
```

#### Arguments

- `uuid` (required) —

#### Examples

!!! warning "Example needed"
    No curated input/output example for this command yet.
