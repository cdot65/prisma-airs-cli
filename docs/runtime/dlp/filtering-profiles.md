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

List all filtering profiles. Supports pagination, sorting, and status filters.

```bash
airs runtime dlp filtering-profiles list
airs runtime dlp filtering-profiles list --status enabled --page 0 --size 20
airs runtime dlp filtering-profiles list --sort name,asc --output json
```

**Output** — paginated list with total count:

```json
{
  "content": [
    {
      "id": "dfp-001",
      "name": "Outbound-HR",
      "direction": "UPLOAD",
      "log_severity": "HIGH",
      "file_based": true,
      "non_file_based": true,
      "data_profile_id": 1001,
      "audit_metadata": {
        "created_by": "ops@example.com",
        "created_at": "2026-04-12T09:15:00Z",
        "updated_at": "2026-05-01T14:22:00Z"
      }
    }
  ],
  "totalElements": 5,
  "totalPages": 1,
  "number": 0,
  "size": 20
}
```

## get

Retrieve a single filtering profile by ID.

```bash
airs runtime dlp filtering-profiles get dfp-001
airs runtime dlp filtering-profiles get dfp-001 --output json
```

**Output** — full profile object with exception rules and exclusions.

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
  "data_profile_id": 1001,
  "exception_rules": [
    {
      "action": "BLOCK",
      "log_severity": "CRITICAL",
      "data_profile_ids": [1001],
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
airs runtime dlp filtering-profiles replace dfp-001 --body-file dfp-update.json
airs runtime dlp filtering-profiles replace dfp-001 --body-file dfp-update.json --output json
```

**Output** — updated profile with incremented version and refreshed audit metadata.

## Tips

- **Required fields on replace**: `file_based` and `non_file_based` are mandatory in the PUT body; omit the others only if you want them server-side defaulted.
- **Full replacement semantics**: `replace` performs a full PUT, so any field you omit will be cleared. If you need to preserve existing fields, fetch the current profile first, merge your changes, then PUT.
- **Exception rules and exclusions**: Both are optional nested objects. Use exception rules to override matching behavior for specific user groups; use exclusions to pre-filter applications, URLs, or keywords.

## See also

- [Data Profiles](profiles.md) — profiles linked via `data_profile_id`
- [Data Patterns](patterns.md) — patterns embedded in detection rules on profiles
