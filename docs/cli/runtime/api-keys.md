# runtime api-keys

### runtime api-keys list

List API keys

```text
airs runtime api-keys list [options]
```

#### Options

| Flag | Required | Default | Description |
|------|:--------:|---------|-------------|
| `--limit <n>` | No | `100` | Max results |
| `--output <format>` | No | `pretty` | Output format: pretty, table, csv, json, yaml |

#### Examples

*Pretty output (default) — `last8` is the trailing 8 chars of the secret; full key is only echoed on create / regenerate*

```bash
airs runtime api-keys list --limit 2
```

```text
Prisma AIRS — Runtime Configuration
Security profile and topic management


API Keys:

00000000-0000-0000-0000-000000000001
  docs-example-key key: …ABCD1234
00000000-0000-0000-0000-000000000002
  docs-other-example-key key: …EFGH5678
```

*JSON output*

```bash
airs runtime api-keys list --limit 2 --output json
```

```text
[
  {
    "id": "00000000-0000-0000-0000-000000000001",
    "name": "docs-example-key",
    "last8": "ABCD1234",
    "createdAt": "",
    "expiresAt": ""
  },
  {
    "id": "00000000-0000-0000-0000-000000000002",
    "name": "docs-other-example-key",
    "last8": "EFGH5678",
    "createdAt": "",
    "expiresAt": ""
  }
]
```

*YAML output (multi-doc stream — one document per key)*

```bash
airs runtime api-keys list --limit 2 --output yaml
```

```text
id: 00000000-0000-0000-0000-000000000001
name: docs-example-key
last8: ABCD1234
createdAt:
expiresAt:
---
id: 00000000-0000-0000-0000-000000000002
name: docs-other-example-key
last8: EFGH5678
createdAt:
expiresAt:
```

---

### runtime api-keys create

Create a new API key

```text
airs runtime api-keys create [options]
```

#### Options

| Flag | Required | Default | Description |
|------|:--------:|---------|-------------|
| `--config <path>` | Yes | — | JSON file with API key configuration |

#### Examples

*Create a new API key from a config fixture (no JSON output flag — pretty only). The full secret is echoed exactly once; subsequent `list` only shows `last8`.*

```bash
airs runtime api-keys create --config docs/cli/examples/runtime/api-keys-create.json
```

```text
Prisma AIRS — Runtime Configuration
Security profile and topic management

API key created: 00000000-0000-0000-0000-000000000001


API Key Detail:

  ID:      00000000-0000-0000-0000-000000000001
  Name:    docs-example-key
  Key:     EXAMPLEKEYREDACTEDFORDOCSDONOTUSEEXAMPLEKEYREDA
```

---

### runtime api-keys regenerate

Regenerate an API key

```text
airs runtime api-keys regenerate [options] <apiKeyId>
```

#### Arguments

- `apiKeyId` (required) —

#### Options

| Flag | Required | Default | Description |
|------|:--------:|---------|-------------|
| `--interval <n>` | Yes | — | Rotation time interval |
| `--unit <unit>` | Yes | — | Rotation time unit (hours, days, months) |
| `--updated-by <email>` | No | — | Email of user performing regeneration |

#### Examples

*Regenerate by api-key UUID. Returns a NEW UUID and a NEW secret — the old key is invalidated. No JSON output flag — pretty only.*

```bash
airs runtime api-keys regenerate 00000000-0000-0000-0000-000000000001 --interval 90 --unit days --updated-by user@example.com
```

```text
Prisma AIRS — Runtime Configuration
Security profile and topic management

API key regenerated: 00000000-0000-0000-0000-000000000002


API Key Detail:

  ID:      00000000-0000-0000-0000-000000000002
  Name:    docs-example-key
  Key:     EXAMPLEROTATEDKEYREDACTEDFORDOCSDONOTUSEEXAMPLE
```

---

### runtime api-keys delete

Delete an API key

```text
airs runtime api-keys delete [options] <apiKeyName>
```

#### Arguments

- `apiKeyName` (required) —

#### Options

| Flag | Required | Default | Description |
|------|:--------:|---------|-------------|
| `--updated-by <email>` | Yes | — | Email of user performing deletion |

#### Examples

!!! warning "Example needed"
    No curated input/output example for this command yet.
