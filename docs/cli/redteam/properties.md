# redteam properties

### redteam properties list

List property names

```text
airs redteam properties list [options]
```

#### Options

| Flag | Required | Default | Description |
|------|:--------:|---------|-------------|
| `--output <format>` | No | `pretty` | Output format: pretty, table, csv, json, yaml |

#### Examples

*List configured property names (empty state — no properties defined on this tenant)*

```bash
airs redteam properties list
```

```text
Prisma AIRS — AI Red Team
Adversarial scan operations

  No property names found.
```

---

### redteam properties create

Create a property name

```text
airs redteam properties create [options]
```

#### Options

| Flag | Required | Default | Description |
|------|:--------:|---------|-------------|
| `--name <name>` | Yes | — | Property name |

#### Examples

!!! warning "Example needed"
    No curated input/output example for this command yet.

---

### redteam properties values

List values for a property

```text
airs redteam properties values [options] <name>
```

#### Arguments

- `name` (required) —

#### Options

| Flag | Required | Default | Description |
|------|:--------:|---------|-------------|
| `--output <format>` | No | `pretty` | Output format: pretty, json, yaml |

#### Examples

!!! warning "Example needed"
    No curated input/output example for this command yet.

---

### redteam properties add-value

Create a property value

```text
airs redteam properties add-value [options]
```

#### Options

| Flag | Required | Default | Description |
|------|:--------:|---------|-------------|
| `--name <name>` | Yes | — | Property name |
| `--value <value>` | Yes | — | Property value |

#### Examples

!!! warning "Example needed"
    No curated input/output example for this command yet.
