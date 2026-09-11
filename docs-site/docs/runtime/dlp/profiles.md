---
title: Data Profiles
---

# Data Profiles

Manage Data Profiles on the DLP service. Profiles define detection rules using `expression_tree` or `multi_profile`. The contract exposes create/read/update but no supported DELETE; status-based retirement is not live-verified.

:::warning[September 6 live SDK validation]
Advanced-profile creation succeeded, but PATCH/PUT retirement returned HTTP 500. OPTIONS advertised DELETE, yet an owned-fixture DELETE returned HTTP 501. One unbound test profile remains journaled and active. Do not treat a status patch as proven cleanup or repeat creates while cleanup is failing. See the SDK's [captured example results](https://cdot65.github.io/prisma-airs-sdk/guides/examples) for the workflow and its explicit limitations.
:::

## Commands

| Command | Description | Exit Code |
|---------|-------------|-----------|
| `list` | List all data profiles with optional pagination and sorting | 1 on error |
| `create` | Create a new profile | 1 on error |
| `get` | Fetch a single profile by ID | 1 on error |
| `replace` | Full PUT: update all fields of a profile | 1 on error |
| `patch` | JSON Merge Patch: update only specified fields | 1 on error |
| `delete` | **Stub** — explains the unsupported cleanup path; sends no API request | always 2 |

## list

List tenant-created profiles with optional pagination and sorting. Predefined
(PANW-shipped) profiles are hidden by default; add `--include-predefined` to show
them.

```bash
airs runtime dlp profiles list
airs runtime dlp profiles list --limit 50 --offset 0 --sort name,asc --output json
```

**Output (`--output json`)** — a bare array of complete camelCase records:

```json
[
    {
      "id": "1234567890",
      "name": "EU-Regulated (umbrella)",
      "type": "custom",
      "profileType": "advanced",
      "status": "active",
      "version": 1
    }
]
```

Use `get <id>` for nested fields (`detectionRules` with `expressionTree` or
`multiProfile`, `auditMetadata`, and server-rendered
`advanceDataPatternsRuleRequest`).

:::note[Nullable fields]
Underlying API `expression_tree` responses are recursive — many nodes carry `null` for `operator_type`, `rule_item`, or `sub_expressions`. CLI v4 uses `@cdot65/prisma-airs-sdk@^0.18.0` to parse them.
:::

## create

`--name` is required. `--profile-type` defaults to `advanced`. Basic writes are rejected before any API call: on September 11, the API returned an advanced profile when a basic profile was requested. This guard applies to create, replace, and patch, including raw JSON bodies. For the common case — a flat boolean of pattern IDs — pass `--pattern-id <id>` repeatedly and (optionally) `--combinator and|or|not|and_not|or_not` (default `or`):

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
| `--name <s>` | Required (unless supplied in the body); at most 32 characters |
| `--profile-type <s>` | `advanced` (default); `basic` writes are unsupported |
| `--description <s>` | Optional |
| `--granular` | Mark as granular data profile |
| `--pattern-id <id>` | Repeatable; each is resolved with GET and becomes `expression_tree.sub_expressions[].rule_item` |
| `--combinator <op>` | `or` (default), `and`, `not`, `and_not`, `or_not` |
| `--confidence <level>` | `low`, `medium`, or `high` (default); must be supported by each referenced pattern |

Profile names are limited to **32 characters**, including any restore name prefix.
An identical-body live probe on September 11 succeeded at 32 characters and returned HTTP 400
at 33. The published SDK schema allows 64, but the CLI enforces the observed 32-character
limit before OAuth for flags and raw create/replace/patch bodies. See the
[recorded boundary evidence](transfer.md).

Before writing, the CLI fetches every distinct `--pattern-id` from the selected tenant.
Each leaf carries the fetched ID, name, version, detection technique, and supported confidence
levels, with `match_type: include` and `occurrence_count >= 1`. Missing, inactive, or incomplete
references stop the command before POST/PUT. Regex and weighted-regex references retain their
own techniques. This rule shape was verified live on September 11; see the
[transfer acceptance log](transfer.md). API failures exit 1; invalid inputs exit 2.

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

For nested `expression_tree` (AND-of-ORs etc.) or `multi_profile` composition, pass JSON. Raw bodies are sent as supplied; use IDs, names, versions, and techniques from the selected tenant. Replace the placeholders below with current pattern metadata before running the example:

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
          { "rule_item": { "id": "<first-pattern-id>", "name": "<first-pattern-name>", "version": 1,
                           "detection_technique": "regex", "match_type": "include",
                           "confidence_level": "high",
                           "occurrence_operator_type": "more_than_equal_to",
                           "occurrence_count": 1 } },
          { "rule_item": { "id": "<second-pattern-id>", "name": "<second-pattern-name>", "version": 1,
                           "detection_technique": "weighted_regex", "match_type": "include",
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

Retrieve a single profile by ID. CLI v4 uses `@cdot65/prisma-airs-sdk@^0.18.0`.

```bash
airs runtime dlp profiles get 11995028
airs runtime dlp profiles get 11995028 --output json
```

**Pretty output:**

```
  Data Profile:

    ID            11995028
    Name          U.K. PIOCP
    Description   Default profile for U.K. PIOCP
    Type          predefined
    Profile Type  basic
    Status        active
    Version       1
    Updated       2026-05-15T08:05:35.633Z
```

**JSON output:**

```json
{
  "id": "11995028",
  "name": "U.K. PIOCP",
  "description": "Default profile for U.K. PIOCP",
  "type": "predefined",
  "profile_type": "basic",
  "status": "active",
  "version": 1,
  "updated": "2026-05-15T08:05:35.633Z"
}
```

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

# Attempt lifecycle retirement only after verifying support in your environment.
# The latest live test returned HTTP 500; this is not a verified cleanup workflow.
airs runtime dlp profiles patch 1234567890 \
  --set name='"High-risk PII"' \
  --set profile_type='"advanced"' \
  --set profile_status='"deleted"'
```

`--body-file` is mutually exclusive with `--set/--clear`. Values are coerced (numbers, booleans, `null`, JSON literals). Quote to force strings: `--set count='"5"'`.

**Output (`--output json`)** — curated ack `{action: "patched", id, name, type, status, version}`.

## delete

Stub command — the supplied DLP contract does not expose DELETE. It explains the cleanup limitation, sends no API request, and exits **2**, which is not a successful deletion:

```bash
airs runtime dlp profiles delete 1234567890
# exit code: 2
```

The following historical status-patch idiom is **not live-verified**. The September 6 check returned HTTP 500 and left the owned profile active. Inspect the current state before any environment-specific investigation:

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
- **No supported DELETE**: Do not assume `profile_status: 'deleted'` was applied. A successful response followed by an independent state check is required before claiming retirement.

## See also

- [Data Patterns](patterns.md) — patterns referenced in expression tree leaves
- [Data Dictionaries](dictionaries.md) — keyword lists for `detection_technique: 'dictionary'` leaves
- [Data Filtering Profiles](filtering-profiles.md) — binds profiles to scanning policy via `data_profile_id`
