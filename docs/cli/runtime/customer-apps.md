# runtime customer-apps

### runtime customer-apps list

List customer apps

```text
airs runtime customer-apps list [options]
```

#### Options

| Flag | Required | Default | Description |
|------|:--------:|---------|-------------|
| `--limit <n>` | No | `100` | Max results |
| `--output <format>` | No | `pretty` | Output format: pretty, table, csv, json, yaml |

#### Examples

*Pretty output (default) — only `name` is rendered; richer detail requires `customer-apps get` (currently blocked, see cdot65/prisma-airs-cli#115)*

```bash
airs runtime customer-apps list --limit 2
```

```text
Prisma AIRS — Runtime Configuration
Security profile and topic management


Customer Apps:

  example-app
  example-other-app
```

*JSON output — `id` and `description` are empty in the list response (the list endpoint only returns names)*

```bash
airs runtime customer-apps list --limit 2 --output json
```

```text
[
  {
    "id": "",
    "name": "example-app",
    "description": ""
  },
  {
    "id": "",
    "name": "example-other-app",
    "description": ""
  }
]
```

*YAML output (multi-doc stream — one document per app)*

```bash
airs runtime customer-apps list --limit 2 --output yaml
```

```text
id:
name: example-app
description:
---
id:
name: example-other-app
description:
```

---

### runtime customer-apps get

Get customer app details

```text
airs runtime customer-apps get [options] <appName>
```

#### Arguments

- `appName` (required) —

#### Examples

!!! warning "Example needed"
    No curated input/output example for this command yet.

---

### runtime customer-apps update

Update a customer app

```text
airs runtime customer-apps update [options] <appId>
```

#### Arguments

- `appId` (required) —

#### Options

| Flag | Required | Default | Description |
|------|:--------:|---------|-------------|
| `--config <path>` | Yes | — | JSON file with app updates |

#### Examples

!!! warning "Example needed"
    No curated input/output example for this command yet.

---

### runtime customer-apps delete

Delete a customer app

```text
airs runtime customer-apps delete [options] <appName>
```

#### Arguments

- `appName` (required) —

#### Options

| Flag | Required | Default | Description |
|------|:--------:|---------|-------------|
| `--updated-by <email>` | Yes | — | Email of user performing deletion |

#### Examples

!!! warning "Example needed"
    No curated input/output example for this command yet.
