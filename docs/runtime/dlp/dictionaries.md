---
title: Data Dictionaries
---

# Data Dictionaries

Manage Dictionaries on the DLP service. Dictionaries provide keyword-list-driven detection for DLP patterns. Create and replace use multipart upload (metadata + keyword file). Full CRUD is available: list, create, get, replace, patch, delete.

## Commands

| Command | Description | Exit Code |
|---------|-------------|-----------|
| `list` | List all dictionaries with optional keyword inclusion | 1 on error |
| `create` | Create a new dictionary (multipart: metadata + keyword file) | 1 on error |
| `get` | Fetch a single dictionary by ID | 1 on error |
| `replace` | Full multipart replace of metadata and keyword file | 1 on error |
| `patch` | JSON Merge Patch: update only specified metadata fields | 1 on error |
| `delete` | Delete a dictionary | 1 on error |

## list

List all dictionaries with optional pagination.

```bash
airs runtime dlp dictionaries list
airs runtime dlp dictionaries list --page 0 --size 50 --output json
airs runtime dlp dictionaries list --keywords  # Include keyword array in output
```

**Output** — paginated list of dictionary objects; `keywords` array populated only if requested.

## create

Create a new dictionary. Requires multipart: metadata JSON + keyword file (newline-delimited text).

First, create the keyword file `codenames.txt`:

```
alpha
bravo
charlie
delta
echo
```

Then create the metadata file `dict-meta.json`:

```json
{
  "category": "Confidential",
  "name": "project-codenames",
  "original_file_name": "codenames.txt",
  "region_name": "us-west-2",
  "description": "Internal project codenames — phonetic alphabet",
  "is_case_sensitive": false
}
```

Then invoke create:

```bash
airs runtime dlp dictionaries create --metadata-file dict-meta.json --file-path codenames.txt
airs runtime dlp dictionaries create --metadata-file dict-meta.json --file-path codenames.txt --output json
airs runtime dlp dictionaries create --metadata-file dict-meta.json --file-path codenames.txt --keywords
```

**Output** — created dictionary with server-assigned `id` and lifecycle stamps. If `--keywords` is passed, the `keywords[]` array is included showing all parsed entries.

## get

Retrieve a single dictionary by ID.

```bash
airs runtime dlp dictionaries get dict-7f30c2
airs runtime dlp dictionaries get dict-7f30c2 --output json
airs runtime dlp dictionaries get dict-7f30c2 --keywords  # Include keyword array
```

**Output** — full dictionary object with metadata, keyword count, and optionally the keyword array.

## replace

Perform a full multipart replace of both metadata and keyword file. The API may return 200+body (some regions) or 204+empty (others).

Create updated metadata `dict-meta-v2.json`:

```json
{
  "category": "Confidential",
  "name": "project-codenames",
  "original_file_name": "codenames.txt",
  "region_name": "us-west-2",
  "description": "Internal project codenames — updated",
  "is_case_sensitive": false
}
```

Create updated keyword file `codenames-v2.txt`:

```
alpha
bravo
charlie
delta
echo
foxtrot
```

Then invoke replace:

```bash
airs runtime dlp dictionaries replace dict-7f30c2 --metadata-file dict-meta-v2.json --file-path codenames-v2.txt
airs runtime dlp dictionaries replace dict-7f30c2 --metadata-file dict-meta-v2.json --file-path codenames-v2.txt --output json
```

**Output** — updated dictionary with incremented keyword count and refreshed `audit_metadata`. If the API returns 204, the output is empty; always re-fetch with `get --keywords` to canonically observe state.

## patch

Use JSON Merge Patch to update only metadata fields. Required fields even on patch: `category`, `name`, `original_file_name`. Other fields use nullable semantics: omit to leave unchanged, send `null` to clear.

Create a patch file `dict-patch.json`:

```json
{
  "category": "Confidential",
  "name": "project-codenames-v2",
  "original_file_name": "codenames.txt",
  "description": null
}
```

Then invoke patch:

```bash
airs runtime dlp dictionaries patch dict-7f30c2 --body-file dict-patch.json
airs runtime dlp dictionaries patch dict-7f30c2 --body-file dict-patch.json --output json
```

**Output** — patched dictionary with `description` cleared (omitted from response) and the new name persisted. Keywords are not affected by PATCH — use REPLACE to change the keyword file.

## delete

Delete a dictionary.

```bash
airs runtime dlp dictionaries delete dict-7f30c2
```

**Exit code** — 0 on success, 1 on error.

## Tips

- **Multipart upload**: CREATE and REPLACE require two files: metadata (JSON) and keyword file (plain text, newline-delimited). The CLI combines them into a multipart body; do not set `Content-Type` manually.
- **200 vs 204 on replace**: The DLP API may return 200+body or 204+empty depending on region/configuration. The replace command handles both. After replace, always re-fetch via `get --keywords` to canonically observe the updated state.
- **Keyword file format**: Keywords must be newline-delimited. Trailing newline is optional but recommended. Empty lines are typically ignored server-side.
- **Category values**: Valid categories are `Academic`, `Confidential`, `Employment`, `Financial`, `Government`, `Healthcare`, `Legal`, `Marketing`, `Source Code` (note the space in the last one).
- **Patch vs Replace**: Use PATCH to update metadata only (name, description, is_case_sensitive). Use REPLACE if you need to change the keyword file or region.

## See also

- [Data Profiles](profiles.md) — profiles use `detection_technique: 'dictionary'` to reference dictionary ids
- [Data Patterns](patterns.md) — alternative detection surface (regex / weighted_regex / EDM)
- [Data Filtering Profiles](filtering-profiles.md) — binds profiles to scanning policy
