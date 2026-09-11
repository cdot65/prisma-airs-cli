---
title: Data Dictionaries
---

# Data Dictionaries

:::tip[Live creation verified — September 11, 2026]
Custom dictionary creation works with the SCM region display name, such as `United States`.
The earlier detail-free HTTP 400 was reproduced with region codes (`GLOBAL`, `us-west-2`);
it was not evidence of a missing entitlement. The SDK's existing multipart `json` Blob plus
`file` upload works without classification or tags. See the [transfer acceptance log](transfer.md)
for prod → dev round-trip evidence.
:::

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

List tenant-created dictionaries with optional pagination. Predefined
(PANW-shipped) dictionaries are hidden by default; add `--include-predefined` to
show them.

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
  --region "United States" \
  --description "Internal project codenames — phonetic alphabet" \
  --file codenames.txt \
  --include-keywords
```

Flag reference:

| Flag | Notes |
|------|-------|
| `--file <path>` | **Required** — TXT: one keyword per line; CSV: one header row followed by keywords |
| `--name <s>` | Dictionary name |
| `--category <s>` | `Academic`, `Confidential`, `Employment`, `Financial`, `Government`, `Healthcare`, `Legal`, `Marketing`, `Source Code` |
| `--region <s>` | Exact SCM region display name, e.g. `United States`; region codes are not equivalent |
| `--description <s>` | Optional |
| `--classification <s>` | Legacy top-level metadata field; server support unverified. Does not become `tags.classification`; not required for creation |
| `--metadata-file <path>` | JSON metadata file (overrides flat flags) |
| `--include-keywords` | Echo parsed `keywords[]` in response |

**Output (`--output json`)** — curated ack `{action: "created", id, name, type, status, version}`. Add `--include-keywords` to attach the parsed keyword list to the underlying SDK response (visible via `get <id> --keywords --output json` after create).

:::note[Dictionary validation errors]
API failures exit **1**. Safe problem-details fields are shown when available, for example
`originalFileName: must not be blank`. A detail-free HTTP 400 includes a reminder to check
the SCM region display name; it does not assert a license or entitlement problem.
Invalid flags and malformed metadata JSON exit **2** before upload. Error output never
includes keyword file content or rejected payload values. With `--debug`, DLP logs retain
HTTP methods, URLs, status codes, timing, and redacted headers while omitting all request and
response bodies. `PANW_AI_SEC_DEBUG_BODY` is disabled for DLP commands as well.
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
  --region "United States" \
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
- **Keyword file format**: For TXT, put one keyword on each line. For CSV, include a header row followed by one keyword per line: the service treats the first row as a header and does not store it as a keyword. The live CSV parser preserves literal quote characters; do not apply CSV quoting or escaping to plain keyword rows. Comma-containing CSV keywords were rejected by the live API. Live probes on September 11 preserved all three TXT keywords but dropped the first keyword of a headerless CSV. Re-fetch with `get <id> --keywords --output json` to verify the stored keyword list after upload.
- **Category values**: Valid categories are `Academic`, `Confidential`, `Employment`, `Financial`, `Government`, `Healthcare`, `Legal`, `Marketing`, `Source Code` (note the space in the last one).
- **Patch vs Replace**: Use PATCH to update metadata only (name, description, is_case_sensitive). Use REPLACE if you need to change the keyword file or region.

## See also

- [Data Profiles](profiles.md) — profiles use `detection_technique: 'dictionary'` to reference dictionary ids
- [Data Patterns](patterns.md) — alternative detection surface (regex / weighted_regex / EDM)
- [Data Filtering Profiles](filtering-profiles.md) — binds profiles to scanning policy
