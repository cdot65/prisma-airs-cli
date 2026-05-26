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

*Append labels to a scan (preserves existing keys)*

```bash
airs model-security labels add 00000000-0000-0000-0000-000000000002 --labels '[{"key":"env","value":"docs-test"},{"key":"capture","value":"true"}]'
```

```text
Prisma AIRS — Model Security
ML model supply chain security

Labels added.
```

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

*Replace all labels on a scan (drops keys not in the request)*

```bash
airs model-security labels set 00000000-0000-0000-0000-000000000002 --labels '[{"key":"env","value":"updated"}]'
```

```text
Prisma AIRS — Model Security
ML model supply chain security

Labels set.
```

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

*Delete one or more labels by comma-separated key list*

```bash
airs model-security labels delete 00000000-0000-0000-0000-000000000002 --keys env
```

```text
Prisma AIRS — Model Security
ML model supply chain security

Labels deleted.
```

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
