# model-security labels

### model-security labels add

Add labels to a scan

```text
airs model-security labels add [options] <scanUuid>
```

#### Arguments

- `scanUuid` (required) —

#### Options

| Flag | Required | Default | Description |
|------|:--------:|---------|-------------|
| `--labels <json>` | Yes | — | JSON array of {key, value} labels |

#### Examples

!!! warning "Example needed"
    No curated input/output example for this command yet.

---

### model-security labels set

Replace all labels on a scan

```text
airs model-security labels set [options] <scanUuid>
```

#### Arguments

- `scanUuid` (required) —

#### Options

| Flag | Required | Default | Description |
|------|:--------:|---------|-------------|
| `--labels <json>` | Yes | — | JSON array of {key, value} labels |

#### Examples

!!! warning "Example needed"
    No curated input/output example for this command yet.

---

### model-security labels delete

Delete labels from a scan by key

```text
airs model-security labels delete [options] <scanUuid>
```

#### Arguments

- `scanUuid` (required) —

#### Options

| Flag | Required | Default | Description |
|------|:--------:|---------|-------------|
| `--keys <keys>` | Yes | — | Comma-separated label keys to delete |

#### Examples

!!! warning "Example needed"
    No curated input/output example for this command yet.

---

### model-security labels keys

List available label keys

```text
airs model-security labels keys [options]
```

#### Options

| Flag | Required | Default | Description |
|------|:--------:|---------|-------------|
| `--limit <n>` | No | `20` | Max results |

#### Examples

*Empty result — no scan labels have been created yet*

```bash
airs model-security labels keys
```

```text
Prisma AIRS — Model Security
ML model supply chain security

No label keys found.
```

---

### model-security labels values

List values for a label key

```text
airs model-security labels values [options] <key>
```

#### Arguments

- `key` (required) —

#### Options

| Flag | Required | Default | Description |
|------|:--------:|---------|-------------|
| `--limit <n>` | No | `20` | Max results |

#### Examples

*Empty result — no values for an unknown key*

```bash
airs model-security labels values example-key
```

```text
Prisma AIRS — Model Security
ML model supply chain security

No values for key "example-key".
```
