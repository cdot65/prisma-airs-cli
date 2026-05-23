---
title: Data Filtering Profiles
---

# Data Filtering Profiles

Manage Data Filtering Profiles on the DLP service. Filtering profiles define scan behavior across file and non-file content (chat, prompts). The underlying API exposes read + full-replace only — create and delete are not available. To onboard a new profile, provision it via the Strata Cloud Manager UI first, then manage it through the CLI.

## Commands

| Command | Description | Exit Code |
|---------|-------------|-----------|
| `list` | List all data filtering profiles with optional filters | 1 on error |
| `get` | Fetch a single profile by ID | 1 on error |
| `replace` | Full PUT: update all fields of a profile | 1 on error |

## list

List all filtering profiles. Supports pagination and sorting.

```bash
airs runtime dlp filtering-profiles list
airs runtime dlp filtering-profiles list --page 0 --size 20
airs runtime dlp filtering-profiles list --sort name,asc --output json
```

**Output** — Spring `Page<>` envelope, sample with one entry (`file_type` truncated; `totalElements`/`totalPages` are emitted as `null` by this endpoint):

```json
{
  "content": [
    {
      "id": "6a10...",
      "name": "asdfafdsadsa",
      "description": null,
      "tenant_id": "<TENANT_ID>",
      "type": "custom",
      "data_profile_id": 1234567890,
      "direction": "c2s",
      "file_based": true,
      "non_file_based": false,
      "log_severity": "low",
      "scan_type": "include",
      "is_end_user_coaching_enabled": null,
      "is_granular_profile": false,
      "is_parent_managed": false,
      "euc_template_id": null,
      "version": 1,
      "file_type": ["csv", "doc", "docx", "pdf", "txt-upload", "xlsx", "7z"],
      "audit_metadata": {
        "created_at": 1779473698140,
        "created_by": null,
        "updated_at": 1779473698140,
        "updated_by": "Strata Cloud Manager"
      },
      "criteria_details": [],
      "exception_rules": [],
      "exclusions": {
        "app_exclusion_list": [],
        "url_exclusion_list": [],
        "exclusion_list": {}
      },
      "rule1": {
        "action": "alert",
        "response_page": "This file has dlp issues",
        "show_rsp_page": "no"
      },
      "rule2": null
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
    The DLP API returns `null` for several fields on real tenants — including `description`, `rule2`, `audit_metadata.created_by`, and `is_end_user_coaching_enabled`. Make sure your downstream parsing accepts these. (CLI requires `@cdot65/prisma-airs-sdk@^0.9.2` or newer for full nullable coverage.)

## get

Retrieve a single filtering profile by ID.

```bash
airs runtime dlp filtering-profiles get 6a10...
airs runtime dlp filtering-profiles get 6a10... --output json
```

**Output** — same shape as a single `content[]` entry from `list`:

```json
{
  "id": "6a10...",
  "name": "asdfafdsadsa",
  "description": null,
  "tenant_id": "<TENANT_ID>",
  "type": "custom",
  "data_profile_id": 1234567890,
  "direction": "c2s",
  "file_based": true,
  "non_file_based": false,
  "log_severity": "low",
  "scan_type": "include",
  "version": 1,
  "audit_metadata": {
    "created_at": 1779473698140,
    "created_by": null,
    "updated_at": 1779473698140,
    "updated_by": "Strata Cloud Manager"
  },
  "criteria_details": [],
  "exception_rules": [],
  "exclusions": {
    "app_exclusion_list": [],
    "url_exclusion_list": [],
    "exclusion_list": {}
  },
  "rule1": {
    "action": "alert",
    "response_page": "This file has dlp issues",
    "show_rsp_page": "no"
  },
  "rule2": null
}
```

## replace

Perform a full PUT to update a filtering profile. All fields in the request body are treated as the complete desired state — existing fields not re-sent will be cleared.

Create a file `dfp-update.json`:

```json
{
  "file_based": true,
  "non_file_based": true,
  "description": "Updated HR data filtering",
  "direction": "UPLOAD",
  "log_severity": "CRITICAL",
  "data_profile_id": 1234567890,
  "exception_rules": [
    {
      "action": "BLOCK",
      "log_severity": "CRITICAL",
      "data_profile_ids": [1234567890],
      "source_attributes": {
        "match_any": false,
        "user_group_ids": ["legal-review"]
      }
    }
  ]
}
```

Then invoke replace:

```bash
airs runtime dlp filtering-profiles replace 6a10... --body-file dfp-update.json
airs runtime dlp filtering-profiles replace 6a10... --body-file dfp-update.json --output json
```

**Output** — updated profile with incremented version and refreshed audit metadata.

## Tips

- **Required fields on replace**: `file_based` and `non_file_based` are mandatory in the PUT body; omit the others only if you want them server-side defaulted.
- **Full replacement semantics**: `replace` performs a full PUT, so any field you omit will be cleared. If you need to preserve existing fields, fetch the current profile first, merge your changes, then PUT.
- **Exception rules and exclusions**: Both are optional nested objects. Use exception rules to override matching behavior for specific user groups; use exclusions to pre-filter applications, URLs, or keywords.

## See also

- [Data Profiles](profiles.md) — profiles linked via `data_profile_id`
- [Data Patterns](patterns.md) — patterns embedded in detection rules on profiles
