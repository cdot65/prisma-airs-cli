# runtime dlp filtering-profiles

### runtime dlp filtering-profiles list

List filtering profiles

```text
airs runtime dlp filtering-profiles list [options]
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
airs runtime dlp filtering-profiles list --size 2 --sort name,asc
```

```text
Data Filtering Profiles:

000000000000000000000001
  Bulk CCN  predefined dir:c2s sev:low v1
000000000000000000000002
  CCPA  predefined dir:c2s sev:low v1

page=0 size=2 returned=2 total=29
```

*JSON output*

```bash
airs runtime dlp filtering-profiles list --size 2 --sort name,asc --output json
```

```text
{
  "items": [
    {
      "id": "000000000000000000000001",
      "name": "Bulk CCN",
      "type": "predefined",
      "direction": "c2s",
      "severity": "low",
      "version": 1
    },
    {
      "id": "000000000000000000000002",
      "name": "CCPA",
      "type": "predefined",
      "direction": "c2s",
      "severity": "low",
      "version": 1
    }
  ],
  "page": {
    "number": 0,
    "size": 2,
    "total": 29,
    "returned": 2
  }
}
```

*YAML output*

```bash
airs runtime dlp filtering-profiles list --size 2 --sort name,asc --output yaml
```

```text
items:
  - id: '000000000000000000000001'
    name: Bulk CCN
    type: predefined
    direction: c2s
    severity: low
    version: 1
  - id: '000000000000000000000002'
    name: CCPA
    type: predefined
    direction: c2s
    severity: low
    version: 1
page:
  number: 0
  size: 2
  total: 29
  returned: 2
```

---

### runtime dlp filtering-profiles get

Get a filtering profile by id

```text
airs runtime dlp filtering-profiles get [options] <id>
```

#### Arguments

- `id` (required) —

#### Options

| Flag | Required | Default | Description |
|------|:--------:|---------|-------------|
| `--output <fmt>` | No | `pretty` | Output format |

#### Examples

*Pretty output (default — predefined filtering profile)*

```bash
airs runtime dlp filtering-profiles get 000000000000000000000001
```

```text
Data Filtering Profile:

  ID              000000000000000000000001
  Name            Bulk CCN
  Type            predefined
  Data Profile    00000001
  Direction       c2s
  Severity        low
  Scan Type       include
  File Based      yes
  Non-File Based  no
  Version         1
  File Types      35
  Updated         2026-05-15T08:05:35.599Z
```

*JSON output*

```bash
airs runtime dlp filtering-profiles get 000000000000000000000001 --output json
```

```text
{
  "id": "000000000000000000000001",
  "name": "Bulk CCN",
  "type": "predefined",
  "data_profile": 1,
  "direction": "c2s",
  "severity": "low",
  "scan_type": "include",
  "file_based": "yes",
  "non_file_based": "no",
  "version": 1,
  "file_types": 35,
  "updated": "2026-05-15T08:05:35.599Z"
}
```

*YAML output*

```bash
airs runtime dlp filtering-profiles get 000000000000000000000001 --output yaml
```

```text
id: 000000000000000000000001
name: Bulk CCN
type: predefined
data_profile: 1
direction: c2s
severity: low
scan_type: include
file_based: 'yes'
non_file_based: 'no'
version: 1
file_types: 35
updated: '2026-05-15T08:05:35.599Z'
```

---

### runtime dlp filtering-profiles replace

Full-replace a filtering profile (PUT)

```text
airs runtime dlp filtering-profiles replace [options] <id>
```

#### Arguments

- `id` (required) —

#### Options

| Flag | Required | Default | Description |
|------|:--------:|---------|-------------|
| `--file-based` | No | — | Apply to file-based scans (boolean) |
| `--non-file-based` | No | — | Apply to non-file-based scans (boolean) |
| `--description <s>` | No | — | Description |
| `--direction <s>` | No | — | Direction: BOTH|UPLOAD|DOWNLOAD |
| `--log-severity <s>` | No | — | Severity: CRITICAL|HIGH|MEDIUM|LOW|INFORMATIONAL |
| `--scan-type <s>` | No | — | Scan type: include|exclude |
| `--data-profile-id <n>` | No | — | Data profile ID |
| `--euc-template-id <s>` | No | — | EUC template ID |
| `--end-user-coaching` | No | — | Enable end-user coaching |
| `--granular` | No | — | Granular profile |
| `--file-type <s>` | No | — | File type (repeatable) |
| `--body <json|->` | No | — | Raw JSON body (escape hatch; or "-" for stdin) |
| `--body-file <path>` | No | — | Raw JSON body file (escape hatch) |
| `--output <fmt>` | No | `pretty` | Output format |

#### Examples

*Replace a filtering profile from a body fixture (see docs/cli/examples/dlp/filtering-profiles/replace.json). The API returns the new version on success.*

```bash
airs runtime dlp filtering-profiles replace 000000000000000000000001 --body-file docs/cli/examples/dlp/filtering-profiles/replace.json --output json
```

```text
{
  "action": "replaced",
  "id": "000000000000000000000001",
  "name": "example-filtering-profile",
  "type": "predefined",
  "version": 2
}
```
