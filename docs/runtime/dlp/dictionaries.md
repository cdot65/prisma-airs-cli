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

**Output** — Spring `Page<>` envelope (`totalElements`/`totalPages` are emitted as `null` by this endpoint); example with one predefined entry:

```json
{
  "content": [
    {
      "id": "6901...",
      "name": "Bank Names",
      "description": "List of large international banks",
      "category": "Finance",
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
      "tags": { "classification": ["pab"] },
      "attributes": null,
      "audit_metadata": {
        "created_at": 1761657319491,
        "created_by": "System",
        "updated_at": 1761657319491,
        "updated_by": "System"
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

!!! note
    `keywords` is `null` unless `--keywords` is passed. `detection_sub_technique`, `attributes`, and the `audit_metadata.created_by`/`updated_by` slots are commonly `null` on predefined entries.

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
airs runtime dlp dictionaries get 6901...
airs runtime dlp dictionaries get 6901... --output json
airs runtime dlp dictionaries get 6901... --keywords  # Include keyword array
```

**Output** — full dictionary object:

```json
{
  "id": "6901...",
  "name": "Bank Names",
  "description": "List of large international banks",
  "category": "Finance",
  "region_name": "GLOBAL",
  "type": "predefined",
  "is_case_sensitive": false,
  "is_parent_managed": false,
  "detection_technique": "dictionary",
  "detection_sub_technique": null,
  "dictionary_metadata": {
    "number_of_keywords": 200,
    "original_file_name": "",
    "original_file_size_in_byte": 0
  },
  "keywords": null,
  "tags": { "classification": ["pab"] },
  "attributes": null,
  "audit_metadata": {
    "created_at": 1761657319491,
    "created_by": null,
    "updated_at": 1761657319491,
    "updated_by": null
  }
}
```

Note `dictionary_metadata.number_of_keywords` reflects the canonical server-side count even when `keywords` is `null`. Pass `--keywords` to populate the array.

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
airs runtime dlp dictionaries replace 6901... --metadata-file dict-meta-v2.json --file-path codenames-v2.txt
airs runtime dlp dictionaries replace 6901... --metadata-file dict-meta-v2.json --file-path codenames-v2.txt --output json
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
airs runtime dlp dictionaries patch 6901... --body-file dict-patch.json
airs runtime dlp dictionaries patch 6901... --body-file dict-patch.json --output json
```

**Output** — patched dictionary with `description` cleared (omitted from response) and the new name persisted. Keywords are not affected by PATCH — use REPLACE to change the keyword file.

## delete

Delete a dictionary.

```bash
airs runtime dlp dictionaries delete 6901...
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
