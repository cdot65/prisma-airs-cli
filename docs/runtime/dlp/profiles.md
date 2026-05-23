---
title: Data Profiles
---

# Data Profiles

Manage Data Profiles on the DLP service. Profiles define detection rules using two rule types: `expression_tree` (recursive boolean logic over detection techniques) and `multi_profile` (composition of other profiles). CRUD is available except DELETE — profiles are archived by patching `profile_status`.

## Commands

| Command | Description | Exit Code |
|---------|-------------|-----------|
| `list` | List all data profiles with optional pagination and sorting | 1 on error |
| `create` | Create a new profile | 1 on error |
| `get` | Fetch a single profile by ID | 1 on error |
| `replace` | Full PUT: update all fields of a profile | 1 on error |
| `patch` | JSON Merge Patch: update only specified fields | 1 on error |

## list

List all profiles with optional pagination and sorting.

```bash
airs runtime dlp profiles list
airs runtime dlp profiles list --page 0 --size 50 --sort name,asc --output json
```

**Output** — Spring `Page<>` envelope; example showing a `multi_profile` entry (server returns the composed pattern set echoed in `advance_data_patterns_rule_request`):

```json
{
  "content": [
    {
      "id": "1234567890",
      "name": "EU-Regulated (umbrella)",
      "description": null,
      "tenant_id": "<TENANT_ID>",
      "type": "custom",
      "profile_status": "active",
      "profile_type": "advanced",
      "is_granular_data_profile": false,
      "is_parent_managed": false,
      "version": 1,
      "advance_data_patterns_rule_request": [
        "(... server-rendered detection expression, truncated ...)"
      ],
      "detection_rules": [
        {
          "rule_type": "multi_profile",
          "multi_profile": {
            "data_profile_ids": [1234567891, 1234567892, 1234567893],
            "operator_type": "or"
          }
        }
      ],
      "audit_metadata": {
        "created_at": 1779473698140,
        "created_by": "Strata Cloud Manager",
        "updated_at": 1779473698140,
        "updated_by": "Strata Cloud Manager"
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

Example `expression_tree` entry (truncated to one leaf for brevity — real responses nest several layers of `sub_expressions`):

```json
{
  "id": "1234567890",
  "name": "InfoSec - Code Assistant - Strict",
  "profile_type": "advanced",
  "version": 5,
  "detection_rules": [
    {
      "rule_type": "expression_tree",
      "expression_tree": {
        "operator_type": "or",
        "rule_item": null,
        "sub_expressions": [
          {
            "operator_type": null,
            "rule_item": {
              "detection_technique": "regex",
              "id": "6900...",
              "name": "Bank - ABA Routing Number",
              "version": 1,
              "by_unique_count": false,
              "confidence_level": "low",
              "supported_confidence_levels": ["high", "low"],
              "occurrence_operator_type": "any"
            },
            "sub_expressions": []
          }
        ]
      }
    }
  ]
}
```

!!! note "Nullable fields"
    The `expression_tree` is recursive and many nodes legitimately carry `null` values — particularly `operator_type`, `rule_item`, and (at intermediate nodes) `sub_expressions` slots. CLI requires `@cdot65/prisma-airs-sdk@^0.9.2` or newer to parse this surface; older SDK pins fail Zod validation on real responses.

## create

Create a new data profile with `expression_tree` or `multi_profile` rules. Required fields: `name`, `detection_rules`.

### Example: expression_tree (AND logic)

Create a file `profile-expr.json`:

```json
{
  "name": "High-risk PII (SSN AND CC)",
  "description": "Fires only when both SSN and CC pattern leaves match",
  "detection_rules": [
    {
      "rule_type": "expression_tree",
      "expression_tree": {
        "operator_type": "and",
        "sub_expressions": [
          {
            "rule_item": {
              "detection_technique": "regex",
              "match_type": "include",
              "confidence_level": "high",
              "occurrence_operator_type": "more_than_equal_to",
              "occurrence_count": 1
            }
          },
          {
            "rule_item": {
              "detection_technique": "weighted_regex",
              "match_type": "include",
              "confidence_level": "high",
              "occurrence_operator_type": "more_than_equal_to",
              "occurrence_count": 1
            }
          }
        ]
      }
    }
  ]
}
```

Then invoke create:

```bash
airs runtime dlp profiles create --body-file profile-expr.json
airs runtime dlp profiles create --body-file profile-expr.json --output json
```

### Example: multi_profile (OR composition)

Create a file `profile-multi.json`:

```json
{
  "name": "EU-Regulated (umbrella)",
  "description": "GDPR-PII OR EU-Healthcare",
  "detection_rules": [
    {
      "rule_type": "multi_profile",
      "multi_profile": {
        "operator_type": "or",
        "data_profile_ids": [1234567891, 1234567892]
      }
    }
  ]
}
```

Then invoke create:

```bash
airs runtime dlp profiles create --body-file profile-multi.json --output json
```

**Output** — created profile with server-assigned `id`, `profile_status: 'active'`, `version: 1`, and `audit_metadata`. Multi-profile compositions auto-promote `profile_type` to `'advanced'`.

## get

Retrieve a single profile by ID.

```bash
airs runtime dlp profiles get 1234567890
airs runtime dlp profiles get 1234567890 --output json
```

**Output** — same shape as a single `content[]` entry from `list`.

!!! warning "Known issue (2026-05-23)"
    The DLP API currently returns HTTP 400 for `GET /v2/api/data-profiles/{id}` against live tenants, even with valid IDs from the `list` response. This is a server-side issue, not a CLI or SDK bug — reproducible via `curl` with the same credentials. Use `list` and filter client-side until the upstream is fixed.

## replace

Perform a full PUT to update a profile. The entire body is treated as the desired state.

Create a file `profile-update.json` with all required fields:

```json
{
  "name": "High-risk PII (SSN AND CC)",
  "description": "Updated: requires both SSN and CC with high confidence",
  "detection_rules": [
    {
      "rule_type": "expression_tree",
      "expression_tree": {
        "operator_type": "and",
        "sub_expressions": [
          {
            "rule_item": {
              "detection_technique": "regex",
              "match_type": "include",
              "confidence_level": "high",
              "occurrence_operator_type": "more_than_equal_to",
              "occurrence_count": 1
            }
          },
          {
            "rule_item": {
              "detection_technique": "weighted_regex",
              "match_type": "include",
              "confidence_level": "high",
              "occurrence_operator_type": "more_than_equal_to",
              "occurrence_count": 2
            }
          }
        ]
      }
    }
  ]
}
```

Then invoke replace:

```bash
airs runtime dlp profiles replace 1234567890 --body-file profile-update.json
airs runtime dlp profiles replace 1234567890 --body-file profile-update.json --output json
```

**Output** — updated profile with incremented version and refreshed `audit_metadata`.

## patch

Use JSON Merge Patch to update only specified fields. Required fields even on patch: `name` and `profile_type`. Other fields use nullable semantics: omit to leave unchanged, send `null` to clear.

Create a file `profile-patch.json`:

```json
{
  "name": "High-risk PII (SSN AND CC)",
  "profile_type": "advanced",
  "description": "Patched description without touching detection_rules"
}
```

Then invoke patch:

```bash
airs runtime dlp profiles patch 1234567890 --body-file profile-patch.json
airs runtime dlp profiles patch 1234567890 --body-file profile-patch.json --output json
```

**Output** — patched profile with only the specified fields updated; detection rules preserved as-is.

## Tips

- **Expression tree nesting**: Build complex detection logic using `and` / `or` operators with nested `sub_expressions` and leaf `rule_item` nodes. Each leaf carries the detection technique and technique-specific thresholds.
- **Multi-profile composition**: Use `multi_profile` to combine multiple existing profiles with a single operator (`and` or `or`). The composed profile auto-promotes to `profile_type: 'advanced'` server-side.
- **Merge Patch semantics**: On PATCH, `name` and `profile_type` are required. Arrays like `detection_rules` are replaced wholesale if sent; omit to preserve. Send `null` to clear optional fields like `description`.
- **No DELETE**: Profiles cannot be deleted via API. To archive, PATCH with `profile_status: 'deleted'` if the API supports it, or use the Strata Cloud Manager UI.

## See also

- [Data Patterns](patterns.md) — patterns referenced in expression tree leaves
- [Data Dictionaries](dictionaries.md) — keyword lists for `detection_technique: 'dictionary'` leaves
- [Data Filtering Profiles](filtering-profiles.md) — binds profiles to scanning policy via `data_profile_id`
