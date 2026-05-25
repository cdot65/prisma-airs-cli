# runtime dlp profiles

### runtime dlp profiles list

List data profiles

```text
airs runtime dlp profiles list [options]
```

#### Options

| Flag | Required | Default | Description |
|------|:--------:|---------|-------------|
| `--page <n>` | No | — | Zero-indexed page number |
| `--size <n>` | No | — | Page size |
| `--sort <field,dir>` | No | — | Sort criteria (repeatable) |
| `--output <fmt>` | No | `pretty` | Output format |

#### Examples

*Pretty output (default — DLP profiles list has no custom renderer, so pretty == JSON)*

```bash
airs runtime dlp profiles list --size 2 --sort id,asc
```

```text
{
  "content": [
    {
      "id": "00000001",
      "name": "Bulk CCN",
      "description": "Default profile for Bulk CCN",
      "tenant_id": "<tenant-id>",
      "type": "predefined",
      "profile_status": "active",
      "profile_type": "basic",
      "is_granular_data_profile": false,
      "is_parent_managed": false,
      "version": 1,
      "advance_data_patterns_rule_request": null,
      "detection_rules": null,
      "audit_metadata": {
        "created_at": 1761657319421,
        "created_by": "System",
        "updated_at": 1761657319421,
        "updated_by": "System"
      }
    },
    {
      "id": "00000002",
      "name": "CCPA",
      "description": "Default profile for CCPA",
      "tenant_id": "<tenant-id>",
      "type": "predefined",
      "profile_status": "active",
      "profile_type": "advanced",
      "is_granular_data_profile": false,
      "is_parent_managed": false,
      "version": 1,
      "advance_data_patterns_rule_request": [
        "(Tax Id - US - TIN high more_than_equal_to 1) or (Credit Card Number high more_than_equal_to 1 and Magnetic Stripe Information high more_than_equal_to 1) or (National Id - US Social Security Number - SSN high more_than_equal_to 1)"
      ],
      "detection_rules": null,
      "audit_metadata": {
        "created_at": 1761657319427,
        "created_by": "System",
        "updated_at": 1761657319427,
        "updated_by": "System"
      }
    }
  ],
  "page": {
    "number": 0,
    "size": 2,
    "total_pages": 15,
    "total_elements": 30,
    "number_of_elements": 2,
    "first": true,
    "last": false,
    "empty": false
  }
}
```

*JSON output*

```bash
airs runtime dlp profiles list --size 2 --sort id,asc --output json
```

```text
{
  "content": [
    {
      "id": "00000001",
      "name": "Bulk CCN",
      "description": "Default profile for Bulk CCN",
      "tenant_id": "<tenant-id>",
      "type": "predefined",
      "profile_status": "active",
      "profile_type": "basic",
      "is_granular_data_profile": false,
      "is_parent_managed": false,
      "version": 1,
      "advance_data_patterns_rule_request": null,
      "detection_rules": null,
      "audit_metadata": {
        "created_at": 1761657319421,
        "created_by": "System",
        "updated_at": 1761657319421,
        "updated_by": "System"
      }
    },
    {
      "id": "00000002",
      "name": "CCPA",
      "description": "Default profile for CCPA",
      "tenant_id": "<tenant-id>",
      "type": "predefined",
      "profile_status": "active",
      "profile_type": "advanced",
      "is_granular_data_profile": false,
      "is_parent_managed": false,
      "version": 1,
      "advance_data_patterns_rule_request": [
        "(Tax Id - US - TIN high more_than_equal_to 1) or (Credit Card Number high more_than_equal_to 1 and Magnetic Stripe Information high more_than_equal_to 1) or (National Id - US Social Security Number - SSN high more_than_equal_to 1)"
      ],
      "detection_rules": null,
      "audit_metadata": {
        "created_at": 1761657319427,
        "created_by": "System",
        "updated_at": 1761657319427,
        "updated_by": "System"
      }
    }
  ],
  "page": {
    "number": 0,
    "size": 2,
    "total_pages": 15,
    "total_elements": 30,
    "number_of_elements": 2,
    "first": true,
    "last": false,
    "empty": false
  }
}
```

*YAML output*

```bash
airs runtime dlp profiles list --size 2 --sort id,asc --output yaml
```

```text
content:
  - id: '00000001'
    name: Bulk CCN
    description: Default profile for Bulk CCN
    tenant_id: <tenant-id>
    type: predefined
    profile_status: active
    profile_type: basic
    is_granular_data_profile: false
    is_parent_managed: false
    version: 1
    advance_data_patterns_rule_request: null
    detection_rules: null
    audit_metadata:
      created_at: 1761657319421
      created_by: System
      updated_at: 1761657319421
      updated_by: System
  - id: '00000002'
    name: CCPA
    description: Default profile for CCPA
    tenant_id: <tenant-id>
    type: predefined
    profile_status: active
    profile_type: advanced
    is_granular_data_profile: false
    is_parent_managed: false
    version: 1
    advance_data_patterns_rule_request:
      - >-
        (Tax Id - US - TIN high more_than_equal_to 1) or (Credit Card
        Number high more_than_equal_to 1 and Magnetic Stripe Information
        high more_than_equal_to 1) or (National Id - US Social Security
        Number - SSN high more_than_equal_to 1)
    detection_rules: null
    audit_metadata:
      created_at: 1761657319427
      created_by: System
      updated_at: 1761657319427
      updated_by: System
page:
  number: 0
  size: 2
  total_pages: 15
  total_elements: 30
  number_of_elements: 2
  first: true
  last: false
  empty: false
```

---

### runtime dlp profiles create

Create a data profile

```text
airs runtime dlp profiles create [options]
```

#### Options

| Flag | Required | Default | Description |
|------|:--------:|---------|-------------|
| `--name <s>` | No | — | Profile name (required unless --body-file) |
| `--profile-type <s>` | No | — | Profile type: basic|advanced (default: advanced) |
| `--description <s>` | No | — | Description |
| `--granular` | No | — | Granular data profile |
| `--pattern-id <id>` | No | — | Data pattern ID to include (repeatable). Builds a simple expression_tree. |
| `--combinator <op>` | No | — | Combinator for --pattern-id: or|and|not|and_not|or_not (default: or) |
| `--confidence <level>` | No | — | Confidence level for --pattern-id leaves (default: high) |
| `--body <json|->` | No | — | Raw JSON body (escape hatch; or "-" for stdin) |
| `--body-file <path>` | No | — | Raw JSON body file (escape hatch; required for complex rule trees) |
| `--output <fmt>` | No | `pretty` | Output format |

#### Examples

!!! warning "Example needed"
    No curated input/output example for this command yet.

---

### runtime dlp profiles get

Get a data profile by id

```text
airs runtime dlp profiles get [options] <id>
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

### runtime dlp profiles replace

Full-replace a data profile (PUT)

```text
airs runtime dlp profiles replace [options] <id>
```

#### Arguments

- `id` (required) —

#### Options

| Flag | Required | Default | Description |
|------|:--------:|---------|-------------|
| `--name <s>` | No | — | Profile name (required unless --body-file) |
| `--profile-type <s>` | No | — | Profile type: basic|advanced (default: advanced) |
| `--description <s>` | No | — | Description |
| `--granular` | No | — | Granular data profile |
| `--pattern-id <id>` | No | — | Data pattern ID to include (repeatable). Builds a simple expression_tree. |
| `--combinator <op>` | No | — | Combinator for --pattern-id: or|and|not|and_not|or_not (default: or) |
| `--confidence <level>` | No | — | Confidence level for --pattern-id leaves (default: high) |
| `--body <json|->` | No | — | Raw JSON body (escape hatch; or "-" for stdin) |
| `--body-file <path>` | No | — | Raw JSON body file (escape hatch; required for complex rule trees) |
| `--output <fmt>` | No | `pretty` | Output format |

#### Examples

!!! warning "Example needed"
    No curated input/output example for this command yet.

---

### runtime dlp profiles patch

JSON Merge Patch. body must include name + profile_type. Use --body-file for nested fields. --set/--clear coerce values: numbers/booleans/JSON literals. To force a string, quote: --set count='"5"'.

```text
airs runtime dlp profiles patch [options] <id>
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

### runtime dlp profiles delete

Not supported — prints the patch idiom and exits 2

```text
airs runtime dlp profiles delete [options] <id>
```

#### Arguments

- `id` (required) —

#### Examples

*Stub — DLP data profiles have no DELETE endpoint; soft-delete via patch*

```bash
airs runtime dlp profiles delete 00000001
```

```text
This DLP API has no DELETE for data profiles.
To soft-delete, fetch the profile to get its name + profile_type, then patch:

  airs runtime dlp profiles get 00000001 --output json
  airs runtime dlp profiles patch 00000001 --set profile_status='"deleted"' \
    --set name='"<existing-name>"' --set profile_type='"<existing-type>"'
```
