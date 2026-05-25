# runtime dlp dictionaries

### runtime dlp dictionaries list

List dictionaries

```text
airs runtime dlp dictionaries list [options]
```

#### Options

| Flag | Required | Default | Description |
|------|:--------:|---------|-------------|
| `--page <n>` | No | — | — |
| `--size <n>` | No | — | — |
| `--sort <field,dir>` | No | — | (repeatable) |
| `--keywords` | No | — | Include keyword list in response |
| `--include-keywords` | No | — | Alias for --keywords |
| `--output <fmt>` | No | `pretty` | Output format |

#### Examples

*Pretty output (default — JSON envelope)*

```bash
airs runtime dlp dictionaries list --size 2 --sort name,asc
```

```text
{
  "content": [
    {
      "id": "000000000000000000000001",
      "name": "Allergy",
      "description": "Most common allergies",
      "category": "Medical",
      "region_name": "GLOBAL",
      "type": "predefined",
      "is_case_sensitive": false,
      "is_parent_managed": false,
      "detection_technique": "dictionary",
      "detection_sub_technique": null,
      "dictionary_metadata": {
        "number_of_keywords": 0,
        "original_file_name": "",
        "original_file_size_in_byte": 0
      },
      "keywords": null,
      "tags": {
        "classification": [
          "pab"
        ]
      },
      "attributes": null,
      "audit_metadata": {
        "created_at": 1761657319488,
        "created_by": "System",
        "updated_at": 1761657319488,
        "updated_by": "System"
      }
    },
    {
      "id": "000000000000000000000002",
      "name": "Auto Insurance Providers",
      "description": "List of most used auto insurance providers",
      "category": "Insurance Providers",
      "region_name": "GLOBAL",
      "type": "predefined",
      "is_case_sensitive": false,
      "is_parent_managed": false,
      "detection_technique": "dictionary",
      "detection_sub_technique": null,
      "dictionary_metadata": {
        "number_of_keywords": 0,
        "original_file_name": "",
        "original_file_size_in_byte": 0
      },
      "keywords": null,
      "tags": {
        "classification": [
          "pab"
        ]
      },
      "attributes": null,
      "audit_metadata": {
        "created_at": 1761657319489,
        "created_by": "System",
        "updated_at": 1761657319489,
        "updated_by": "System"
      }
    }
  ],
  "empty": false,
  "first": true,
  "last": false,
  "number": 0,
  "pageable": {
    "offset": 0,
    "paged": true,
    "unpaged": false,
    "sort": {
      "empty": true,
      "sorted": false,
      "unsorted": true
    },
    "page_number": 0,
    "page_size": 2
  },
  "size": 2,
  "sort": {
    "empty": true,
    "sorted": false,
    "unsorted": true
  },
  "total_pages": 16,
  "number_of_elements": 2,
  "total_elements": 31
}
```

*JSON output (explicit)*

```bash
airs runtime dlp dictionaries list --size 2 --sort name,asc --output json
```

```text
{
  "content": [
    {
      "id": "000000000000000000000001",
      "name": "Allergy",
      "description": "Most common allergies",
      "category": "Medical",
      "region_name": "GLOBAL",
      "type": "predefined",
      "is_case_sensitive": false,
      "is_parent_managed": false,
      "detection_technique": "dictionary",
      "detection_sub_technique": null,
      "dictionary_metadata": {
        "number_of_keywords": 0,
        "original_file_name": "",
        "original_file_size_in_byte": 0
      },
      "keywords": null,
      "tags": {
        "classification": [
          "pab"
        ]
      },
      "attributes": null,
      "audit_metadata": {
        "created_at": 1761657319488,
        "created_by": "System",
        "updated_at": 1761657319488,
        "updated_by": "System"
      }
    },
    {
      "id": "000000000000000000000002",
      "name": "Auto Insurance Providers",
      "description": "List of most used auto insurance providers",
      "category": "Insurance Providers",
      "region_name": "GLOBAL",
      "type": "predefined",
      "is_case_sensitive": false,
      "is_parent_managed": false,
      "detection_technique": "dictionary",
      "detection_sub_technique": null,
      "dictionary_metadata": {
        "number_of_keywords": 0,
        "original_file_name": "",
        "original_file_size_in_byte": 0
      },
      "keywords": null,
      "tags": {
        "classification": [
          "pab"
        ]
      },
      "attributes": null,
      "audit_metadata": {
        "created_at": 1761657319489,
        "created_by": "System",
        "updated_at": 1761657319489,
        "updated_by": "System"
      }
    }
  ],
  "empty": false,
  "first": true,
  "last": false,
  "number": 0,
  "pageable": {
    "offset": 0,
    "paged": true,
    "unpaged": false,
    "sort": {
      "empty": true,
      "sorted": false,
      "unsorted": true
    },
    "page_number": 0,
    "page_size": 2
  },
  "size": 2,
  "sort": {
    "empty": true,
    "sorted": false,
    "unsorted": true
  },
  "total_pages": 16,
  "number_of_elements": 2,
  "total_elements": 31
}
```

*YAML output*

```bash
airs runtime dlp dictionaries list --size 2 --sort name,asc --output yaml
```

```text
content:
  - id: 000000000000000000000001
    name: Allergy
    description: Most common allergies
    category: Medical
    region_name: GLOBAL
    type: predefined
    is_case_sensitive: false
    is_parent_managed: false
    detection_technique: dictionary
    detection_sub_technique: null
    dictionary_metadata:
      number_of_keywords: 0
      original_file_name: ''
      original_file_size_in_byte: 0
    keywords: null
    tags:
      classification:
        - pab
    attributes: null
    audit_metadata:
      created_at: 1761657319488
      created_by: System
      updated_at: 1761657319488
      updated_by: System
  - id: 000000000000000000000002
    name: Auto Insurance Providers
    description: List of most used auto insurance providers
    category: Insurance Providers
    region_name: GLOBAL
    type: predefined
    is_case_sensitive: false
    is_parent_managed: false
    detection_technique: dictionary
    detection_sub_technique: null
    dictionary_metadata:
      number_of_keywords: 0
      original_file_name: ''
      original_file_size_in_byte: 0
    keywords: null
    tags:
      classification:
        - pab
    attributes: null
    audit_metadata:
      created_at: 1761657319489
      created_by: System
      updated_at: 1761657319489
      updated_by: System
empty: false
first: true
last: false
number: 0
pageable:
  offset: 0
  paged: true
  unpaged: false
  sort:
    empty: true
    sorted: false
    unsorted: true
  page_number: 0
  page_size: 2
size: 2
sort:
  empty: true
  sorted: false
  unsorted: true
total_pages: 16
number_of_elements: 2
total_elements: 31
```

---

### runtime dlp dictionaries create

Create dictionary via multipart upload

```text
airs runtime dlp dictionaries create [options]
```

#### Options

| Flag | Required | Default | Description |
|------|:--------:|---------|-------------|
| `--name <s>` | No | — | — |
| `--category <s>` | No | — | — |
| `--region <s>` | No | — | — |
| `--description <s>` | No | — | — |
| `--classification <s>` | No | — | — |
| `--file <path>` | No | — | Keyword file |
| `--metadata-file <path>` | No | — | JSON metadata file (overrides --name/--category/...) |
| `--include-keywords` | No | — | Include keywords in response |
| `--output <fmt>` | No | `pretty` | Output format |

#### Examples

!!! warning "Example needed"
    No curated input/output example for this command yet.

---

### runtime dlp dictionaries get

```text
airs runtime dlp dictionaries get [options] <id>
```

#### Arguments

- `id` (required) —

#### Options

| Flag | Required | Default | Description |
|------|:--------:|---------|-------------|
| `--keywords` | No | — | — |
| `--include-keywords` | No | — | Alias for --keywords |
| `--output <fmt>` | No | `pretty` | Output format |

#### Examples

*Pretty output (default — JSON object)*

```bash
airs runtime dlp dictionaries get 000000000000000000000001
```

```text
{
  "id": "000000000000000000000001",
  "name": "Allergy",
  "description": "Most common allergies",
  "category": "Medical",
  "region_name": "GLOBAL",
  "type": "predefined",
  "is_case_sensitive": false,
  "is_parent_managed": false,
  "detection_technique": "dictionary",
  "detection_sub_technique": null,
  "dictionary_metadata": {
    "number_of_keywords": 85,
    "original_file_name": "",
    "original_file_size_in_byte": 0
  },
  "keywords": null,
  "tags": {
    "classification": [
      "pab"
    ]
  },
  "attributes": null,
  "audit_metadata": {
    "created_at": 1761657319488,
    "created_by": null,
    "updated_at": 1761657319488,
    "updated_by": null
  }
}
```

*JSON output (explicit)*

```bash
airs runtime dlp dictionaries get 000000000000000000000001 --output json
```

```text
{
  "id": "000000000000000000000001",
  "name": "Allergy",
  "description": "Most common allergies",
  "category": "Medical",
  "region_name": "GLOBAL",
  "type": "predefined",
  "is_case_sensitive": false,
  "is_parent_managed": false,
  "detection_technique": "dictionary",
  "detection_sub_technique": null,
  "dictionary_metadata": {
    "number_of_keywords": 85,
    "original_file_name": "",
    "original_file_size_in_byte": 0
  },
  "keywords": null,
  "tags": {
    "classification": [
      "pab"
    ]
  },
  "attributes": null,
  "audit_metadata": {
    "created_at": 1761657319488,
    "created_by": null,
    "updated_at": 1761657319488,
    "updated_by": null
  }
}
```

*YAML output*

```bash
airs runtime dlp dictionaries get 000000000000000000000001 --output yaml
```

```text
id: 000000000000000000000001
name: Allergy
description: Most common allergies
category: Medical
region_name: GLOBAL
type: predefined
is_case_sensitive: false
is_parent_managed: false
detection_technique: dictionary
detection_sub_technique: null
dictionary_metadata:
  number_of_keywords: 85
  original_file_name: ''
  original_file_size_in_byte: 0
keywords: null
tags:
  classification:
    - pab
attributes: null
audit_metadata:
  created_at: 1761657319488
  created_by: null
  updated_at: 1761657319488
  updated_by: null
```

---

### runtime dlp dictionaries replace

Full-replace via multipart upload. --file required. May return 200 (body) or 204 (re-get; falls back to "(state not echoed)" on transient failure).

```text
airs runtime dlp dictionaries replace [options] <id>
```

#### Arguments

- `id` (required) —

#### Options

| Flag | Required | Default | Description |
|------|:--------:|---------|-------------|
| `--name <s>` | No | — | — |
| `--category <s>` | No | — | — |
| `--region <s>` | No | — | — |
| `--description <s>` | No | — | — |
| `--classification <s>` | No | — | — |
| `--file <path>` | No | — | Keyword file (required) |
| `--metadata-file <path>` | No | — | JSON metadata file |
| `--include-keywords` | No | — | — |
| `--output <fmt>` | No | `pretty` | Output format |

#### Examples

!!! warning "Example needed"
    No curated input/output example for this command yet.

---

### runtime dlp dictionaries patch

```text
airs runtime dlp dictionaries patch [options] <id>
```

#### Arguments

- `id` (required) —

#### Options

| Flag | Required | Default | Description |
|------|:--------:|---------|-------------|
| `--body-file <path>` | No | — | — |
| `--set <k=v...>` | No | — | (repeatable) |
| `--clear <key...>` | No | — | (repeatable) |
| `--output <fmt>` | No | `pretty` | Output format |

#### Examples

!!! warning "Example needed"
    No curated input/output example for this command yet.

---

### runtime dlp dictionaries delete

Delete a dictionary

```text
airs runtime dlp dictionaries delete [options] <id>
```

#### Arguments

- `id` (required) —

#### Examples

*Soft-delete a dictionary by id (API returns 204; CLI prints confirmation)*

```bash
airs runtime dlp dictionaries delete 000000000000000000000001
```

```text
deleted 000000000000000000000001
```
