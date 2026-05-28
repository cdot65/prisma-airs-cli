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

**Output (`--output json`)** — curated `{items, page}` projection:

```json
{
  "items": [
    {
      "id": "6a10...",
      "name": "asdfafdsadsa",
      "type": "custom",
      "direction": "c2s",
      "severity": "low",
      "version": 1
    }
  ],
  "page": { "number": 0, "size": 25, "total": 29, "returned": 1 }
}
```

Use `get <id>` for the full nested fields (`file_type`, `criteria_details`, `exception_rules`, `exclusions`, `rule1`, `rule2`, `audit_metadata`).

!!! note "Nullable fields"
    Underlying API responses return `null` for several fields on real tenants — including `description`, `rule2`, `audit_metadata.created_by`, and `is_end_user_coaching_enabled`. CLI requires `@cdot65/prisma-airs-sdk@^0.9.2` or newer.

## get

Retrieve a single filtering profile by ID.

```bash
airs runtime dlp filtering-profiles get 6a146fe17e175b786523c03a
airs runtime dlp filtering-profiles get 6a146fe17e175b786523c03a --output json
```

**Pretty output:**

```
  Data Filtering Profile:

    ID              6a146fe17e175b786523c03a
    Name            SOX
    Type            predefined
    Data Profile    11995027
    Direction       c2s
    Severity        low
    Scan Type       include
    File Based      yes
    Non-File Based  no
    Version         3
    File Types      35
    Updated         2026-05-25T17:06:36.032Z
```

**JSON output:**

```json
{
  "id": "6a10...",
  "name": "asdfafdsadsa",
  "type": "custom",
  "data_profile": 1234567890,
  "direction": "c2s",
  "severity": "low",
  "scan_type": "include",
  "file_based": "yes",
  "non_file_based": "no",
  "version": 1,
  "file_types": 35,
  "updated": "2026-05-25T17:06:36.032Z"
}
```

## replace

Full PUT. `--file-based` and `--non-file-based` are required (both booleans); everything else is optional flat-field flags. For nested `exception_rules` or `exclusions`, use `--body-file`.

```bash
airs runtime dlp filtering-profiles replace 6a10... \
  --file-based --non-file-based \
  --description "Updated HR data filtering" \
  --direction UPLOAD \
  --log-severity CRITICAL \
  --data-profile-id 1234567890 \
  --file-type csv --file-type pdf --file-type docx \
  --output json
```

Flag reference:

| Flag | Notes |
|------|-------|
| `--file-based` / `--non-file-based` | **Required** booleans |
| `--description <s>` | Optional |
| `--direction <s>` | `BOTH`, `UPLOAD`, `DOWNLOAD` |
| `--log-severity <s>` | `CRITICAL`, `HIGH`, `MEDIUM`, `LOW`, `INFORMATIONAL` |
| `--scan-type <s>` | `include`, `exclude` |
| `--data-profile-id <n>` | Numeric profile ID to link |
| `--euc-template-id <s>` | EUC template ID |
| `--end-user-coaching` | Enable EUC |
| `--granular` | Mark as granular profile |
| `--file-type <s>` | Repeatable |

### Escape hatch — `--body-file` for nested rules

For `exception_rules`, `exclusions`, or other nested fields:

```bash
cat > dfp-update.json <<'EOF'
{
  "file_based": true,
  "non_file_based": true,
  "description": "Updated HR data filtering",
  "direction": "UPLOAD",
  "log_severity": "CRITICAL",
  "data_profile_id": 1234567890,
  "exception_rules": [
    { "action": "BLOCK",
      "log_severity": "CRITICAL",
      "data_profile_ids": [1234567890],
      "source_attributes": { "match_any": false, "user_group_ids": ["legal-review"] } }
  ]
}
EOF
airs runtime dlp filtering-profiles replace 6a10... --body-file dfp-update.json --output json
```

**Output (`--output json`)** — curated ack `{action: "replaced", id, name, type, status, version}`.

## Tips

- **Required fields on replace**: `file_based` and `non_file_based` are mandatory in the PUT body; omit the others only if you want them server-side defaulted.
- **Full replacement semantics**: `replace` performs a full PUT, so any field you omit will be cleared. If you need to preserve existing fields, fetch the current profile first, merge your changes, then PUT.
- **Exception rules and exclusions**: Both are optional nested objects. Use exception rules to override matching behavior for specific user groups; use exclusions to pre-filter applications, URLs, or keywords.

## See also

- [Data Profiles](profiles.md) — profiles linked via `data_profile_id`
- [Data Patterns](patterns.md) — patterns embedded in detection rules on profiles
