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
| `delete` | **Stub** — API has no DELETE; prints the patch idiom and exits 2 | always 2 |

## list

List all profiles with optional pagination and sorting.

```bash
airs runtime dlp profiles list
airs runtime dlp profiles list --page 0 --size 50 --sort name,asc --output json
```

**Output (`--output json`)** — curated `{items, page}` projection:

```json
{
  "items": [
    {
      "id": "1234567890",
      "name": "EU-Regulated (umbrella)",
      "type": "custom",
      "profile_type": "advanced",
      "status": "active",
      "version": 1
    }
  ],
  "page": { "number": 0, "size": 25, "total": 30, "returned": 1 }
}
```

Use `get <id>` for nested fields (`detection_rules` with `expression_tree` or `multi_profile`, `audit_metadata`, server-rendered `advance_data_patterns_rule_request`).

!!! note "Nullable fields"
    Underlying API `expression_tree` responses are recursive — many nodes carry `null` for `operator_type`, `rule_item`, or `sub_expressions`. CLI requires `@cdot65/prisma-airs-sdk@^0.9.2` or newer to parse them.

## create

`--name` is required. `--profile-type` defaults to `advanced`. For the common case — a flat boolean of pattern IDs — pass `--pattern-id <id>` repeatedly and (optionally) `--combinator and|or|not|and_not|or_not` (default `or`):

```bash
airs runtime dlp profiles create \
  --name "High-risk PII (SSN OR CC)" \
  --description "Fires on SSN or CC pattern leaves" \
  --pattern-id 6990111aaa \
  --pattern-id 6990222bbb \
  --combinator or \
  --confidence high \
  --output json
```

Flag reference:

| Flag | Notes |
|------|-------|
| `--name <s>` | Required (unless `--body-file`) |
| `--profile-type <s>` | `basic` or `advanced` (default `advanced`) |
| `--description <s>` | Optional |
| `--granular` | Mark as granular data profile |
| `--pattern-id <id>` | Repeatable; each becomes a leaf in `expression_tree.condition_pattern[]` |
| `--combinator <op>` | `or` (default), `and`, `not`, `and_not`, `or_not` |
| `--confidence <level>` | Leaf confidence (default `high`) |

**Output (`--output json`)** — curated ack:

```json
{
  "action": "created",
  "id": "1234567890",
  "name": "High-risk PII (SSN OR CC)",
  "type": "custom",
  "status": "active",
  "version": 1
}
```

### Escape hatch — `--body-file` for complex rules

For nested `expression_tree` (AND-of-ORs etc.) or `multi_profile` composition, pass JSON:

```bash
# expression_tree with AND of two sub-rules
cat > profile-expr.json <<'EOF'
{
  "name": "High-risk PII (SSN AND CC)",
  "profile_type": "advanced",
  "detection_rules": [
    {
      "rule_type": "expression_tree",
      "expression_tree": {
        "operator_type": "and",
        "sub_expressions": [
          { "rule_item": { "detection_technique": "regex", "match_type": "include",
                           "confidence_level": "high",
                           "occurrence_operator_type": "more_than_equal_to",
                           "occurrence_count": 1 } },
          { "rule_item": { "detection_technique": "weighted_regex", "match_type": "include",
                           "confidence_level": "high",
                           "occurrence_operator_type": "more_than_equal_to",
                           "occurrence_count": 1 } }
        ]
      }
    }
  ]
}
EOF
airs runtime dlp profiles create --body-file profile-expr.json --output json

# multi_profile composition
cat > profile-multi.json <<'EOF'
{
  "name": "EU-Regulated (umbrella)",
  "profile_type": "advanced",
  "detection_rules": [
    { "rule_type": "multi_profile",
      "multi_profile": { "operator_type": "or",
                         "data_profile_ids": [1234567891, 1234567892] } }
  ]
}
EOF
airs runtime dlp profiles create --body-file profile-multi.json --output json
```

Multi-profile compositions auto-promote `profile_type` to `advanced`.

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

Full PUT. Same flags as `create`, plus `--body-file` for complex rule trees:

```bash
# Simple flat pattern boolean
airs runtime dlp profiles replace 1234567890 \
  --name "High-risk PII (SSN OR CC)" \
  --pattern-id 6990111aaa --pattern-id 6990222bbb \
  --combinator or --confidence high \
  --output json

# Complex tree
airs runtime dlp profiles replace 1234567890 --body-file profile-update.json --output json
```

**Output (`--output json`)** — curated ack `{action: "replaced", id, name, type, status, version}` with incremented version.

## patch

JSON Merge Patch. Required fields even on patch: `name` and `profile_type` — include via `--set` if patching anything else. Use `--set/--clear` for scalars, `--body-file` for nested rules.

```bash
# Patch description without touching detection_rules
airs runtime dlp profiles patch 1234567890 \
  --set name='"High-risk PII (SSN AND CC)"' \
  --set profile_type='"advanced"' \
  --set description='"Patched description"'

# Soft-delete via profile_status
airs runtime dlp profiles patch 1234567890 \
  --set name='"High-risk PII"' \
  --set profile_type='"advanced"' \
  --set profile_status='"deleted"'
```

`--body-file` is mutually exclusive with `--set/--clear`. Values are coerced (numbers, booleans, `null`, JSON literals). Quote to force strings: `--set count='"5"'`.

**Output (`--output json`)** — curated ack `{action: "patched", id, name, type, status, version}`.

## delete

Stub command — the DLP API does **not** expose DELETE for data profiles. Invoking it prints the soft-delete patch idiom on stderr and exits with code **2**, so scripts can distinguish it from a real success or a transient error (exit 1):

```bash
airs runtime dlp profiles delete 1234567890
# exit code: 2
```

To actually archive a profile, fetch its current `name + profile_type` first, then patch:

```bash
airs runtime dlp profiles get 1234567890 --output json
airs runtime dlp profiles patch 1234567890 \
  --set name='"<existing-name>"' \
  --set profile_type='"<existing-type>"' \
  --set profile_status='"deleted"'
```

The `--body-file` shorthand if you prefer heredoc:

```bash
airs runtime dlp profiles patch 1234567890 --body-file - <<'EOF'
{ "name": "my-profile", "profile_type": "advanced", "profile_status": "deleted" }
EOF
```

## Tips

- **Expression tree nesting**: Build complex detection logic using `and` / `or` operators with nested `sub_expressions` and leaf `rule_item` nodes. Each leaf carries the detection technique and technique-specific thresholds.
- **Multi-profile composition**: Use `multi_profile` to combine multiple existing profiles with a single operator (`and` or `or`). The composed profile auto-promotes to `profile_type: 'advanced'` server-side.
- **Merge Patch semantics**: On PATCH, `name` and `profile_type` are required. Arrays like `detection_rules` are replaced wholesale if sent; omit to preserve. Send `null` to clear optional fields like `description`.
- **No DELETE**: Profiles cannot be deleted via API. To archive, PATCH with `--set profile_status='"deleted"'` (must also include `--set name=...` and `--set profile_type=...`), or use the Strata Cloud Manager UI.

## See also

- [Data Patterns](patterns.md) — patterns referenced in expression tree leaves
- [Data Dictionaries](dictionaries.md) — keyword lists for `detection_technique: 'dictionary'` leaves
- [Data Filtering Profiles](filtering-profiles.md) — binds profiles to scanning policy via `data_profile_id`
