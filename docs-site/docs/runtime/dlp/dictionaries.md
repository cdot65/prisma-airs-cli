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
airs runtime dlp dictionaries list --limit 50 --offset 0 --output json
airs runtime dlp dictionaries list --keywords  # Include keyword array in output
```

**Output (`--output json`)** — a bare array of complete camelCase records.
Optional fields are omitted when the API does not return them:

```json
[
    {
      "id": "6901...",
      "name": "Bank Names",
      "type": "predefined"
    }
]
```

Custom entries with status/version populated include those fields. Pass
`--keywords` when the full keyword list is needed. Use `get <id> --keywords`
for nested fields such as `dictionaryMetadata`, `tags`, and `auditMetadata`.

## create

Multipart upload — keyword file + metadata. Pass metadata via flat flags (preferred) or `--metadata-file`. `--file` is required.

```bash
# Keyword file (newline-delimited)
cat > codenames.txt <<'EOF'
alpha
bravo
charlie
delta
echo
EOF

# Flag-based metadata (preferred)
airs runtime dlp dictionaries create \
  --name "project-codenames" \
  --category Confidential \
  --region us-west-2 \
  --description "Internal project codenames — phonetic alphabet" \
  --file codenames.txt \
  --include-keywords
```

Flag reference:

| Flag | Notes |
|------|-------|
| `--file <path>` | **Required** — keyword file (newline-delimited) |
| `--name <s>` | Dictionary name |
| `--category <s>` | `Academic`, `Confidential`, `Employment`, `Financial`, `Government`, `Healthcare`, `Legal`, `Marketing`, `Source Code` |
| `--region <s>` | Region (e.g. `us-west-2`, `GLOBAL`) |
| `--description <s>` | Optional |
| `--classification <s>` | Tag value (becomes `tags.classification`) |
| `--metadata-file <path>` | JSON metadata file (overrides flat flags) |
| `--include-keywords` | Echo parsed `keywords[]` in response |

**Output (`--output json`)** — curated ack `{action: "created", id, name, type, status, version}`. Add `--include-keywords` to attach the parsed keyword list to the underlying SDK response (visible via `get <id> --keywords --output json` after create).

:::warning[Known upstream issue (2026-05-24)]
The DLP API currently returns generic HTTP 400 on `POST /v2/api/dictionaries` against live tenants. The CLI builds a correctly-formed multipart request — reproducible failure is server-side. Tracked in [cdot65/prisma-airs-sdk#162](https://github.com/cdot65/prisma-airs-sdk/issues/162) / [cdot65/prisma-airs-cli#80](https://github.com/cdot65/prisma-airs-cli/issues/80).
:::

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

Full multipart replace of metadata + keyword file. Same flag set as `create`. The API returns 200+body in some regions, 204+empty in others — the CLI handles both.

```bash
airs runtime dlp dictionaries replace 6901... \
  --name "project-codenames" \
  --category Confidential \
  --region us-west-2 \
  --description "Internal project codenames — updated" \
  --file codenames-v2.txt \
  --output json
```

**Output (`--output json`)** — curated ack `{action: "replaced", id, name, type, status, version}` on 200; `replaced <id> (state not echoed by region)` on 204. After replace, re-fetch via `get --keywords` to canonically observe state.

## patch

JSON Merge Patch. Required fields even on patch: `category`, `name`, `original_file_name` — include via `--set` whenever patching anything else. `--set/--clear` values are coerced (numbers, booleans, `null`, JSON literals); quote to force strings: `--set count='"5"'`.

```bash
# Rename and clear description
airs runtime dlp dictionaries patch 6901... \
  --set name='"project-codenames-v2"' \
  --set category='"Confidential"' \
  --set original_file_name='"codenames.txt"' \
  --clear description \
  --output json

# Or via --body-file for arbitrary metadata
airs runtime dlp dictionaries patch 6901... --body-file dict-patch.json --output json
```

`--body-file` is mutually exclusive with `--set/--clear`. Keywords are not affected by PATCH — use REPLACE to change the keyword file.

**Output (`--output json`)** — curated ack `{action: "patched", id, name, type, status, version}`.

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
