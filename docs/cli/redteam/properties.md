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

*Pretty output (default)*

```bash
airs redteam properties list
```

```text
Prisma AIRS — AI Red Team
Adversarial scan operations


Property Names:

  • severity
  • persona
```

*JSON output (raw string array from SDK 0.10.0)*

```bash
airs redteam properties list --output json
```

```text
[
  "severity",
  "persona"
]
```

*YAML output*

```bash
airs redteam properties list --output yaml
```

```text
- severity
- persona
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

*List values for a property name (pretty)*

```bash
airs redteam properties values severity
```

```text
Prisma AIRS — AI Red Team
Adversarial scan operations


Property Values:

  Property: severity
    • low
    • medium
    • high
```

*JSON output (single {name, values[]} object — SDK 0.10.0 shape)*

```bash
airs redteam properties values severity --output json
```

```text
{
  "name": "severity",
  "values": [
    "low",
    "medium",
    "high"
  ]
}
```

*YAML output*

```bash
airs redteam properties values severity --output yaml
```

```text
name: severity
values:
  - low
  - medium
  - high
```

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

*Create a new value for an existing property name*

```bash
airs redteam properties add-value --name severity --value critical
```

```text
Prisma AIRS — AI Red Team
Adversarial scan operations

Property value 'critical' created successfully
```
