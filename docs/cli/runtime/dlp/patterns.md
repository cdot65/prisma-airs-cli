# runtime dlp patterns

### runtime dlp patterns list

List data patterns

```text
airs runtime dlp patterns list [options]
```

#### Options

| Flag | Required | Default | Description |
|------|:--------:|---------|-------------|
| `--page <n>` | No | — | Zero-indexed page number |
| `--size <n>` | No | — | Page size |
| `--sort <field,dir>` | No | — | Sort criteria (repeatable) |
| `--output <fmt>` | No | `pretty` | Output format |

#### Examples

*Pretty output (default)*

```bash
airs runtime dlp patterns list --size 2 --sort name,asc
```

```text
Data Patterns:

000000000000000000000001
  API Credentials Client ID - Amazon Web Services AWS  predefined  disabled regex v1
000000000000000000000002
  API Credentials Client ID - Bitly  predefined  disabled regex v1

page=0 size=2 returned=2 total=1126
```

*JSON output*

```bash
airs runtime dlp patterns list --size 2 --sort name,asc --output json
```

```text
{
  "items": [
    {
      "id": "000000000000000000000001",
      "name": "API Credentials Client ID - Amazon Web Services AWS",
      "type": "predefined",
      "status": "disabled",
      "technique": "regex",
      "version": 1
    },
    {
      "id": "000000000000000000000002",
      "name": "API Credentials Client ID - Bitly",
      "type": "predefined",
      "status": "disabled",
      "technique": "regex",
      "version": 1
    }
  ],
  "page": {
    "number": 0,
    "size": 2,
    "total": 1126,
    "returned": 2
  }
}
```

*YAML output*

```bash
airs runtime dlp patterns list --size 2 --sort name,asc --output yaml
```

```text
items:
  - id: 000000000000000000000001
    name: API Credentials Client ID - Amazon Web Services AWS
    type: predefined
    status: disabled
    technique: regex
    version: 1
  - id: 000000000000000000000002
    name: API Credentials Client ID - Bitly
    type: predefined
    status: disabled
    technique: regex
    version: 1
page:
  number: 0
  size: 2
  total: 1126
  returned: 2
```

---

### runtime dlp patterns create

Create a data pattern

```text
airs runtime dlp patterns create [options]
```

#### Options

| Flag | Required | Default | Description |
|------|:--------:|---------|-------------|
| `--name <s>` | No | — | Pattern name (required unless --body-file) |
| `--type <s>` | No | — | Pattern type: predefined|custom|file_property (default: custom) |
| `--description <s>` | No | — | Pattern description |
| `--technique <s>` | No | — | Detection technique (default: regex) |
| `--confidence-levels <csv>` | No | — | Confidence levels CSV: e.g. high,low |
| `--regex <pattern>` | No | — | Regex with weight=1 (repeatable) |
| `--weighted-regex <PATTERN|N>` | No | — | Regex with explicit weight (repeatable) |
| `--delimiter <s>` | No | — | Delimiter for proximity matching |
| `--proximity-distance <n>` | No | — | Proximity window (2..1000) |
| `--proximity-keyword <s>` | No | — | Proximity keyword (repeatable) |
| `--tag <k=v>` | No | — | Tag (repeatable, value can be CSV) |
| `--body <json|->` | No | — | Raw JSON body (escape hatch; or "-" for stdin) |
| `--body-file <path>` | No | — | Raw JSON body file (escape hatch) |
| `--output <fmt>` | No | `pretty` | Output format |

#### Examples

*Create a custom regex pattern with --name + --regex flags*

```bash
airs runtime dlp patterns create --name docs-example-pattern --regex '\bACME-\d{6}\b' --output json
```

```text
{
  "action": "created",
  "id": "000000000000000000000003",
  "name": "docs-example-pattern",
  "type": "custom",
  "status": "active",
  "version": 1
}
```

---

### runtime dlp patterns get

Get a data pattern by id

```text
airs runtime dlp patterns get [options] <id>
```

#### Arguments

- `id` (required) —

#### Options

| Flag | Required | Default | Description |
|------|:--------:|---------|-------------|
| `--output <fmt>` | No | `pretty` | Output format |

#### Examples

!!! warning "Example needed"
    No curated input/output example for this command yet.

---

### runtime dlp patterns replace

Full-replace a data pattern (PUT)

```text
airs runtime dlp patterns replace [options] <id>
```

#### Arguments

- `id` (required) —

#### Options

| Flag | Required | Default | Description |
|------|:--------:|---------|-------------|
| `--name <s>` | No | — | Pattern name (required unless --body-file) |
| `--type <s>` | No | — | Pattern type: predefined|custom|file_property (default: custom) |
| `--description <s>` | No | — | Pattern description |
| `--technique <s>` | No | — | Detection technique (default: regex) |
| `--confidence-levels <csv>` | No | — | Confidence levels CSV: e.g. high,low |
| `--regex <pattern>` | No | — | Regex with weight=1 (repeatable) |
| `--weighted-regex <PATTERN|N>` | No | — | Regex with explicit weight (repeatable) |
| `--delimiter <s>` | No | — | Delimiter for proximity matching |
| `--proximity-distance <n>` | No | — | Proximity window (2..1000) |
| `--proximity-keyword <s>` | No | — | Proximity keyword (repeatable) |
| `--tag <k=v>` | No | — | Tag (repeatable, value can be CSV) |
| `--body <json|->` | No | — | Raw JSON body (escape hatch; or "-" for stdin) |
| `--body-file <path>` | No | — | Raw JSON body file (escape hatch) |
| `--output <fmt>` | No | `pretty` | Output format |

#### Examples

!!! warning "Example needed"
    No curated input/output example for this command yet.

---

### runtime dlp patterns patch

JSON Merge Patch. Use --body-file for nested fields. --set/--clear coerce values: numbers/booleans/JSON literals. To force a string, quote: --set count='"5"'.

```text
airs runtime dlp patterns patch [options] <id>
```

#### Arguments

- `id` (required) —

#### Options

| Flag | Required | Default | Description |
|------|:--------:|---------|-------------|
| `--body-file <path>` | No | — | JSON merge-patch body file |
| `--set <k=v...>` | No | — | Set scalar field (repeatable) |
| `--clear <key...>` | No | — | Clear field via merge-patch null (repeatable) |
| `--output <fmt>` | No | `pretty` | Output format |

#### Examples

!!! warning "Example needed"
    No curated input/output example for this command yet.

---

### runtime dlp patterns delete

Soft-delete (archive) a data pattern

```text
airs runtime dlp patterns delete [options] <id>
```

#### Arguments

- `id` (required) —

#### Examples

*Soft-delete (archive) a data pattern by id*

```bash
airs runtime dlp patterns delete 000000000000000000000003
```

```text
archived 000000000000000000000003
```
