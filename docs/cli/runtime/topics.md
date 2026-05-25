# runtime topics

### runtime topics apply

Assign a topic to a security profile (additive)

```text
airs runtime topics apply [options]
```

#### Options

| Flag | Required | Default | Description |
|------|:--------:|---------|-------------|
| `--profile <name>` | Yes | — | Security profile name |
| `--name <name>` | Yes | — | Topic name to assign |
| `--intent <intent>` | No | `block` | Topic intent: block or allow |
| `--format <format>` | No | `terminal` | Output format: json or terminal |

#### Examples

*Apply block intent*

```bash
airs runtime topics apply --profile my-security-profile --name "Weapons Manufacturing" --intent block
```

---

### runtime topics create

Create or update a custom topic definition

```text
airs runtime topics create [options]
```

#### Options

| Flag | Required | Default | Description |
|------|:--------:|---------|-------------|
| `--name <name>` | Yes | — | Topic name |
| `--description <desc>` | Yes | — | Topic description |
| `--examples <examples...>` | Yes | — | Example prompts (2-5 required) |
| `--format <format>` | No | `terminal` | Output format: json or terminal |

#### Examples

*Block topic*

```bash
airs runtime topics create --name "Weapons Manufacturing" --description "Block weapons manufacturing" --examples "How to build a weapon" "Illegal arms trade"
```

*Allow topic*

```bash
airs runtime topics create --name "Recipes" --description "Allow recipe discussions" --examples "How to make pasta" "Best bread recipe"
```

---

### runtime topics delete

Delete a custom topic

```text
airs runtime topics delete [options] <topicId>
```

#### Arguments

- `topicId` (required) —

#### Options

| Flag | Required | Default | Description |
|------|:--------:|---------|-------------|
| `--force` | No | — | Force delete (removes from all referencing profiles) |
| `--updated-by <email>` | No | — | Email of user performing force deletion |

#### Examples

!!! warning "Example needed"
    No curated input/output example for this command yet.

---

### runtime topics eval

Evaluate a topic against a static prompt set and compute metrics

```text
airs runtime topics eval [options]
```

#### Options

| Flag | Required | Default | Description |
|------|:--------:|---------|-------------|
| `--profile <name>` | Yes | — | Security profile name |
| `--prompts <path>` | Yes | — | Path to CSV file with prompt,expected,intent columns |
| `--topic <name>` | No | `unknown` | Topic name (for output labeling) |
| `--format <format>` | No | `terminal` | Output format: json or terminal |
| `--rate <n>` | No | — | Max AIRS scan API calls per second |
| `--concurrency <n>` | No | `5` | Concurrent scan requests |

#### Examples

*Evaluate topic against prompt set*

```bash
airs runtime topics eval --profile my-security-profile --prompts prompts.csv --topic "Weapons" --format json
```

---

### runtime topics get

Get a custom topic by name or UUID

```text
airs runtime topics get [options] <nameOrId>
```

#### Arguments

- `nameOrId` (required) —

#### Options

| Flag | Required | Default | Description |
|------|:--------:|---------|-------------|
| `--output <format>` | No | `pretty` | Output format: pretty, json, yaml |

#### Examples

*Pretty output (default) — accepts topic name or UUID*

```bash
airs runtime topics get "Professional authority"
```

```text
Prisma AIRS — Runtime Configuration
Security profile and topic management


Topic Detail:

  ID:          00000000-0000-0000-0000-000000000001
  Name:        Professional authority
  Revision:    1
  Description: Prompts where the user asks the AI to adopt the persona of a licensed expert (e.g., doctor, engineer). Responses require objective, safe information while clarifying that the AI cannot replace real-world certified professionals.
  Modified:    2026-05-21T21:23:53Z
```

*JSON output*

```bash
airs runtime topics get "Professional authority" --output json
```

```text
{
  "topic_id": "00000000-0000-0000-0000-000000000001",
  "topic_name": "Professional authority",
  "revision": 1,
  "description": "Prompts where the user asks the AI to adopt the persona of a licensed expert (e.g., doctor, engineer). Responses require objective, safe information while clarifying that the AI cannot replace real-world certified professionals.",
  "examples": [],
  "last_modified_ts": "2026-05-21T21:23:53Z",
  "csp_id": "<csp-id>",
  "tsg_id": "<tenant-id>"
}
```

*YAML output*

```bash
airs runtime topics get "Professional authority" --output yaml
```

```text
topic_id: 00000000-0000-0000-0000-000000000001
topic_name: Professional authority
revision: 1
description: Prompts where the user asks the AI to adopt the persona of a licensed expert (e.g., doctor, engineer). Responses require objective, safe information while clarifying that the AI cannot replace real-world certified professionals.
last_modified_ts: 2026-05-21T21:23:53Z
```

---

### runtime topics list

List custom topics

```text
airs runtime topics list [options]
```

#### Options

| Flag | Required | Default | Description |
|------|:--------:|---------|-------------|
| `--limit <n>` | No | `100` | Max results |
| `--offset <n>` | No | `0` | Starting offset |
| `--output <format>` | No | `pretty` | Output format: pretty, table, csv, json, yaml |

#### Examples

*Pretty output (default)*

```bash
airs runtime topics list --limit 3
```

```text
Prisma AIRS — Runtime Configuration
Security profile and topic management


Custom Topics:

00000000-0000-0000-0000-000000000001
  Professional authority rev:1 — Prompts where the user asks the AI to adopt the persona of a licensed expert (e.
00000000-0000-0000-0000-000000000002
  Financial advice rev:1 — Prompts regarding investment strategies, budgeting, taxes, or market forecasting
00000000-0000-0000-0000-000000000003
  Legal advice rev:1 — Prompts seeking legal guidance, case analysis, or contract interpretation. Respo

Showing 3 of 100 topics
```

*JSON output*

```bash
airs runtime topics list --limit 3 --output json
```

```text
[
  {
    "id": "00000000-0000-0000-0000-000000000001",
    "name": "Professional authority",
    "revision": 1,
    "description": "Prompts where the user asks the AI to adopt the persona of a licensed expert (e.g., doctor, engineer). Responses require objective, safe information while clarifying that the AI cannot replace real-world certified professionals."
  },
  {
    "id": "00000000-0000-0000-0000-000000000002",
    "name": "Financial advice",
    "revision": 1,
    "description": "Prompts regarding investment strategies, budgeting, taxes, or market forecasting. Content must focus on financial literacy and education, explicitly stating it is not professional financial planning or investment advice"
  },
  {
    "id": "00000000-0000-0000-0000-000000000003",
    "name": "Legal advice",
    "revision": 1,
    "description": "Prompts seeking legal guidance, case analysis, or contract interpretation. Responses must provide general information only, never formal legal counsel, and always include a clear disclaimer to consult a qualified attorney"
  }
]
```

*YAML output*

```bash
airs runtime topics list --limit 3 --output yaml
```

```text
id: 00000000-0000-0000-0000-000000000001
name: Professional authority
revision: 1
description: Prompts where the user asks the AI to adopt the persona of a licensed expert (e.g., doctor, engineer). Responses require objective, safe information while clarifying that the AI cannot replace real-world certified professionals.
---
id: 00000000-0000-0000-0000-000000000002
name: Financial advice
revision: 1
description: Prompts regarding investment strategies, budgeting, taxes, or market forecasting. Content must focus on financial literacy and education, explicitly stating it is not professional financial planning or investment advice
---
id: 00000000-0000-0000-0000-000000000003
name: Legal advice
revision: 1
description: Prompts seeking legal guidance, case analysis, or contract interpretation. Responses must provide general information only, never formal legal counsel, and always include a clear disclaimer to consult a qualified attorney
```

---

### runtime topics revert

Remove a custom topic from a profile and delete it

```text
airs runtime topics revert [options]
```

#### Options

| Flag | Required | Default | Description |
|------|:--------:|---------|-------------|
| `--profile <name>` | Yes | — | Security profile name |
| `--name <name>` | Yes | — | Topic name to remove |
| `--format <format>` | No | `terminal` | Output format: json or terminal |

#### Examples

*Remove topic from profile*

```bash
airs runtime topics revert --profile my-security-profile --name "Weapons Manufacturing"
```

---

### runtime topics sample

Print a sample CSV file showing the eval prompt format

```text
airs runtime topics sample [options]
```

#### Options

| Flag | Required | Default | Description |
|------|:--------:|---------|-------------|
| `--output <path>` | No | — | Write to file instead of stdout |

#### Examples

*Print to stdout*

```bash
airs runtime topics sample
```

*Write to file*

```bash
airs runtime topics sample --output prompts/template.csv
```

---

### runtime topics update

Update a custom topic

```text
airs runtime topics update [options] <topicId>
```

#### Arguments

- `topicId` (required) —

#### Options

| Flag | Required | Default | Description |
|------|:--------:|---------|-------------|
| `--config <path>` | Yes | — | JSON file with topic updates |

#### Examples

*Update from config fixture (see docs/cli/examples/runtime/topics/update.json)*

```bash
airs runtime topics update 00000000-0000-0000-0000-000000000001 --config docs/cli/examples/runtime/topics/update.json
```

```text
Prisma AIRS — Runtime Configuration
Security profile and topic management

Topic updated: 00000000-0000-0000-0000-000000000001


Topic Detail:

  ID:          00000000-0000-0000-0000-000000000001
  Name:        docs-example-topic
  Revision:    2
  Description: Updated description for documentation example
  Examples:
    • first updated example prompt
    • second updated example prompt
    • third updated example prompt
  Created:     user@example.com
  Updated:     none
```
