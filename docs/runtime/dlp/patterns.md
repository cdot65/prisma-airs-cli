---
title: Data Patterns
---

# Data Patterns

Manage Data Patterns on the DLP service. Patterns define detection techniques (regex, weighted_regex, dictionary, EDM, classifier, etc.) and matching rules. Full CRUD is available: list, create, get, replace (full PUT), patch (JSON Merge Patch), delete.

## Commands

| Command | Description | Exit Code |
|---------|-------------|-----------|
| `list` | List all data patterns with optional pagination and sorting | 1 on error |
| `create` | Create a new pattern | 1 on error |
| `get` | Fetch a single pattern by ID | 1 on error |
| `replace` | Full PUT: update all fields of a pattern | 1 on error |
| `patch` | JSON Merge Patch: update only specified fields | 1 on error |
| `delete` | Soft-delete a pattern (status becomes 'deleted', still resolvable by get) | 1 on error |

## list

List all patterns with optional pagination and sorting.

```bash
airs runtime dlp patterns list
airs runtime dlp patterns list --page 0 --size 50 --sort name,asc --output json
```

**Output** — Spring `Page<>` envelope; example showing both `custom` and `predefined` entries (`totalElements`/`totalPages` are emitted as `null` by this endpoint):

```json
{
  "content": [
    {
      "id": "6990...",
      "name": "IPv4",
      "description": "Just a simple test",
      "tenant_id": "<TENANT_ID>",
      "type": "custom",
      "status": "active",
      "license_type": "standard",
      "is_parent_managed": false,
      "version": 1,
      "detection_config": {
        "technique": "regex",
        "supported_confidence_levels": ["high", "low"]
      },
      "matching_rules": {
        "delimiter": ";",
        "proximity_distance": 200,
        "proximity_keywords": null,
        "regexes": [
          {
            "regex": "\\b(?:25[0-5]|2[0-4]\\d|[01]?\\d\\d?)(?:\\.(?:25[0-5]|2[0-4]\\d|[01]?\\d\\d?)){3}\\b",
            "weight": 1
          }
        ],
        "metadata_criteria": null
      },
      "tags": { "classification": ["pab"] },
      "audit_metadata": {
        "created_at": 1771088671081,
        "created_by": "Strata Cloud Manager",
        "updated_at": 1771088671081,
        "updated_by": "Strata Cloud Manager"
      }
    },
    {
      "id": "6900...",
      "name": "Passport - Australia",
      "type": "predefined",
      "status": "disabled",
      "license_type": "standard",
      "version": 1,
      "detection_config": { "technique": "regex", "supported_confidence_levels": ["high"] },
      "matching_rules": {
        "delimiter": null,
        "proximity_keywords": ["passport", "passport no"],
        "regexes": [{ "regex": "...", "weight": 1 }],
        "metadata_criteria": null
      }
    }
  ],
  "pageable": { "page_number": 0, "page_size": 25, "offset": 0, "paged": true, "unpaged": false },
  "first": true,
  "last": false,
  "size": 25,
  "number": 0,
  "number_of_elements": 25,
  "empty": false,
  "totalElements": null,
  "totalPages": null
}
```

!!! note "Nullable fields"
    Real responses include several `null` values on `matching_rules` nested fields — `delimiter`, `proximity_keywords`, `regexes`, `metadata_criteria` are each independently nullable depending on the detection technique. CLI requires `@cdot65/prisma-airs-sdk@^0.9.2` or newer to parse this surface; older SDK pins fail Zod validation.

## create

Create a new data pattern. Required fields: `name`, `type`, `detection_config`.

Create a file `pattern.json`:

```json
{
  "name": "cc-numbers-weighted",
  "type": "custom",
  "description": "Credit-card numbers, weighted by proximity to card-related keywords",
  "detection_config": {
    "technique": "weighted_regex",
    "supported_confidence_levels": ["low", "medium", "high"]
  },
  "matching_rules": {
    "proximity_distance": 30,
    "proximity_keywords": ["card", "credit", "visa", "mastercard", "amex"],
    "regexes": [
      { "regex": "\\b\\d{16}\\b", "weight": 1.0 },
      { "regex": "\\b\\d{15}\\b", "weight": 0.8 }
    ]
  },
  "tags": {
    "classification": ["PCI"],
    "compliance": ["PCI-DSS-3.2.1"],
    "geography": ["US", "EU"]
  }
}
```

Then invoke create:

```bash
airs runtime dlp patterns create --body-file pattern.json
airs runtime dlp patterns create --body-file pattern.json --output json
```

**Output** — created pattern with server-assigned `id`, `status: 'active'`, `version: 1`, and `audit_metadata`.

## get

Retrieve a single pattern by ID.

```bash
airs runtime dlp patterns get 6990...
airs runtime dlp patterns get 6990... --output json
```

**Output** — same shape as a single `content[]` entry from `list`.

!!! warning "Known issue (2026-05-23)"
    The DLP API currently returns HTTP 400 for `GET /v2/api/data-patterns/{id}` against live tenants, even with valid IDs from the `list` response. This is a server-side issue, not a CLI or SDK bug — reproducible via `curl` with the same credentials. Use `list` and filter client-side until the upstream is fixed.

## replace

Perform a full PUT to update a pattern. The entire body is treated as the desired state.

Create a file `pattern-update.json` with all required fields plus any changes:

```json
{
  "name": "cc-numbers-weighted",
  "type": "custom",
  "detection_config": {
    "technique": "weighted_regex",
    "supported_confidence_levels": ["low", "medium", "high"]
  },
  "matching_rules": {
    "proximity_distance": 30,
    "proximity_keywords": ["card", "credit", "visa", "mastercard", "amex"],
    "regexes": [
      { "regex": "\\b\\d{16}\\b", "weight": 1.0 },
      { "regex": "\\b\\d{15}\\b", "weight": 0.8 },
      { "regex": "\\b\\d{13}\\b", "weight": 0.6 }
    ]
  },
  "tags": {
    "classification": ["PCI"],
    "compliance": ["PCI-DSS-3.2.1"],
    "geography": ["US", "EU"]
  }
}
```

Then invoke replace:

```bash
airs runtime dlp patterns replace 6990... --body-file pattern-update.json
airs runtime dlp patterns replace 6990... --body-file pattern-update.json --output json
```

**Output** — updated pattern with incremented version and refreshed `audit_metadata`.

## patch

Use JSON Merge Patch to update only specified fields. Required fields even on patch: `name`, `type`, `detection_config`. Other fields use nullable semantics: omit to leave unchanged, send `null` to clear.

Create a file `pattern-patch.json`:

```json
{
  "name": "cc-numbers-weighted",
  "type": "custom",
  "detection_config": {
    "technique": "weighted_regex",
    "supported_confidence_levels": ["low", "medium", "high"]
  },
  "matching_rules": {
    "proximity_distance": 30,
    "proximity_keywords": ["card", "credit", "visa", "mastercard", "amex"],
    "regexes": [
      { "regex": "\\b\\d{16}\\b", "weight": 1.0 },
      { "regex": "\\b\\d{15}\\b", "weight": 0.8 },
      { "regex": "\\b\\d{13}\\b", "weight": 0.6 }
    ]
  },
  "description": null
}
```

Then invoke patch:

```bash
airs runtime dlp patterns patch 6990... --body-file pattern-patch.json
airs runtime dlp patterns patch 6990... --body-file pattern-patch.json --output json
```

**Output** — patched pattern with `description` cleared (omitted from response) and the third regex persisted.

## delete

Soft-delete a pattern. The pattern becomes invisible to `list` but remains resolvable via `get` with `status: 'deleted'`.

```bash
airs runtime dlp patterns delete 6990...
```

**Exit code** — 0 on success, 1 on error.

## Tips

- **Merge Patch semantics**: On PATCH, `name`, `type`, and `detection_config` are required even if unchanged. Arrays and objects are replaced wholesale (not merged) — re-send the entire `matching_rules` if you modify any part. Omit fields to preserve them; send `null` to clear.
- **Detection techniques**: Valid techniques include `regex`, `weighted_regex`, `dictionary`, `edm`, `document_fingerprint`, `trainable_classifier`, `ml_document`, `ml`, `titus_tag`, `wildfire`, `file_property`, `pab`, and `document_classifier`.
- **Soft delete**: DELETE archives the pattern server-side. Fetching a deleted pattern via `get` returns `status: 'deleted'`.

## See also

- [Data Profiles](profiles.md) — profiles compose patterns via detection rules
- [Data Dictionaries](dictionaries.md) — keyword lists for `dictionary` detection technique
- [Data Filtering Profiles](filtering-profiles.md) — binds profiles to scanning policy
