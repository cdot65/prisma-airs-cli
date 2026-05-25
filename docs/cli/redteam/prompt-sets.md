# redteam prompt-sets

### redteam prompt-sets list

List custom prompt sets

```text
airs redteam prompt-sets list [options]
```

#### Options

| Flag | Required | Default | Description |
|------|:--------:|---------|-------------|
| `--output <format>` | No | `pretty` | Output format: pretty, table, csv, json, yaml |

#### Examples

*List all prompt sets*

```bash
airs redteam prompt-sets list
```

```text
Prompt Sets:

c820d9b8-4342-4d9a-b0b4-6b2d9f5e04fb
  pokemon-guardrail-tests  active
7829805d-6479-4ce1-866b-2bff66a3c766
  daystrom-Explosives and Bomb-Making Discussions-ZdeHhCW  active
d68a14f5-cea3-4047-bedb-ae5726ba20d2
  Saffron  inactive
```

---

### redteam prompt-sets get

Get prompt set details

```text
airs redteam prompt-sets get [options] <uuid>
```

#### Arguments

- `uuid` (required) —

#### Examples

!!! warning "Example needed"
    No curated input/output example for this command yet.

---

### redteam prompt-sets create

Create a new prompt set

```text
airs redteam prompt-sets create [options]
```

#### Options

| Flag | Required | Default | Description |
|------|:--------:|---------|-------------|
| `--name <name>` | Yes | — | Prompt set name |
| `--description <desc>` | No | — | Prompt set description |

#### Examples

!!! warning "Example needed"
    No curated input/output example for this command yet.

---

### redteam prompt-sets update

Update a prompt set

```text
airs redteam prompt-sets update [options] <uuid>
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

### redteam prompt-sets archive

Archive a prompt set

```text
airs redteam prompt-sets archive [options] <uuid>
```

#### Arguments

- `uuid` (required) —

#### Options

| Flag | Required | Default | Description |
|------|:--------:|---------|-------------|
| `--unarchive` | No | — | Unarchive instead |

#### Examples

!!! warning "Example needed"
    No curated input/output example for this command yet.

---

### redteam prompt-sets download

Download CSV template for a prompt set

```text
airs redteam prompt-sets download [options] <uuid>
```

#### Arguments

- `uuid` (required) —

#### Options

| Flag | Required | Default | Description |
|------|:--------:|---------|-------------|
| `--output <path>` | No | — | Output file path |

#### Examples

*Save CSV template (header + one sample row) to a local file*

```bash
airs redteam prompt-sets download 00000000-0000-0000-0000-000000000001 --output template.csv
```

```text
Prisma AIRS — AI Red Team
Adversarial scan operations

Template saved to template.csv

# template.csv:
# prompt,goal
# This is a sample prompt,Optional goal text (leave empty for AI-generated goal)
```

---

### redteam prompt-sets upload

Upload CSV prompts to a prompt set

```text
airs redteam prompt-sets upload [options] <uuid> <file>
```

#### Arguments

- `uuid` (required) —
- `file` (required) —

#### Examples

!!! warning "Example needed"
    No curated input/output example for this command yet.
