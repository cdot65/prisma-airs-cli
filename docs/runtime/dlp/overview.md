---
title: DLP
---

# DLP

`airs runtime dlp` is full CRUD over the four configuration surfaces of the Palo Alto Networks DLP service: data **filtering profiles**, **patterns**, **profiles**, and **dictionaries**. Twenty commands, one shared OAuth token cache, one merge-patch UX across every resource that supports PATCH.

<div class="grid cards" markdown>

-   :material-shield-search:{ .lg .middle } **Filtering Profiles**

    ---

    Bind data profiles to scan policy (file vs non-file, log severity, direction). Read + full-replace only — no create or delete.

    [:octicons-arrow-right-24: Filtering Profiles](filtering-profiles.md)

-   :material-regex:{ .lg .middle } **Patterns**

    ---

    Detection primitives — regex, weighted_regex, dictionary, EDM, classifier. Full CRUD; `delete` is soft (archive).

    [:octicons-arrow-right-24: Patterns](patterns.md)

-   :material-file-tree:{ .lg .middle } **Profiles**

    ---

    Boolean compositions of patterns or other profiles via `expression_tree` / `multi_profile`. No DELETE — soft-delete via `profile_status: "deleted"` patch.

    [:octicons-arrow-right-24: Profiles](profiles.md)

-   :material-book-open-variant:{ .lg .middle } **Dictionaries**

    ---

    Keyword lists for `dictionary`-technique detection. Multipart upload (metadata + keyword file). PUT may return 200 or 204.

    [:octicons-arrow-right-24: Dictionaries](dictionaries.md)

</div>

## Authentication

DLP reuses the AIRS Management OAuth2 credentials — no DLP-specific tokens. A single `getOrCreateManagementClient()` singleton shares the token cache across every `client.dlp.*` call and the existing `runtime` services.

| Variable | Required | What it does |
|----------|:--------:|--------------|
| `PANW_MGMT_CLIENT_ID` | Yes | OAuth2 client ID |
| `PANW_MGMT_CLIENT_SECRET` | Yes | OAuth2 client secret |
| `PANW_MGMT_TSG_ID` | Yes | Tenant Service Group ID |
| `PANW_DLP_ENDPOINT` | -- | Override default DLP base URL (`api.dlp.paloaltonetworks.com`) |

See [Environment Variables](../../reference/environment-variables.md) for the full list.

## Command map

All twenty commands at a glance:

| Resource | list | create | get | replace | patch | delete |
|----------|:----:|:------:|:---:|:-------:|:-----:|:------:|
| [filtering-profiles](filtering-profiles.md) | ✅ | — | ✅ | ✅ | — | — |
| [patterns](patterns.md) | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ soft |
| [profiles](profiles.md) | ✅ | ✅ | ✅ | ✅ | ✅ | stub (exits 2) |
| [dictionaries](dictionaries.md) | ✅ | ✅ multipart | ✅ | ✅ multipart | ✅ | ✅ |

!!! note "Why the gaps"
    `filtering-profiles` and `profiles` API surfaces do not expose DELETE. `filtering-profiles` has no `delete` subcommand at all; `profiles delete <id>` is a stub that prints the patch idiom and exits with code 2. Soft-delete a profile by PATCHing `profile_status: "deleted"`. Patterns soft-delete (archive) on `delete` — the entry stays resolvable via `get` with `status: "deleted"`.

## Resource model

```mermaid
graph LR
  D[Dictionaries] -->|referenced by<br/>technique=dictionary| P[Patterns]
  P -->|expression_tree leaves| Pr[Profiles]
  Pr -->|profile composition| Pr
  Pr -->|data_profile_id| F[Filtering Profiles]
  F -->|scan policy| S[Scanning runtime]
```

Build bottom-up: dictionaries → patterns → profiles → filtering profiles. Soft-delete top-down: profiles archived before patterns; filtering profiles unbind via `data_profile_id`.

## Shared patch UX

All PATCH-capable resources (`patterns`, `profiles`, `dictionaries`) accept the same three input modes — pick whichever fits the shape of your change:

| Mode | When to use | Mutex with |
|------|-------------|------------|
| `--set k=v` (repeatable) | Scalar field tweaks. Values coerce: `true`/`false`, numbers, JSON literals. Quote `'"5"'` to force string-5. `null` rejected — use `--clear` | `--body-file` |
| `--clear k` (repeatable) | Send merge-patch `null` to clear a field | `--body-file` |
| `--body-file <path>` | Nested fields (`detection_rules`, `matching_rules`, etc.). Full JSON merge-patch body, RFC 7396 | `--set` / `--clear` |

**Required fields on patch** vary per resource — merge-patch is RFC 7396 (omit-to-preserve), but the DLP API enforces presence on a small set even when unchanged:

| Resource | Required on every PATCH |
|----------|-------------------------|
| `patterns` | `name`, `type`, `detection_config` |
| `profiles` | `name`, `profile_type` |
| `dictionaries` | `name`, `category`, `original_file_name` |

If you patch anything else, include the required fields via `--set` as well.

## Common gotchas

- **Quote string-5 in `--set`**: `--set count='"5"'` to force a JSON string. `--set count=5` becomes a number; `--set count=true` becomes a boolean.
- **`--set k=null` is rejected** — use `--clear k` instead (the CLI catches this and errors before sending).
- **`profiles delete` is a stub** — exits 2, prints the patch idiom. The real soft-delete:

    ```bash
    airs runtime dlp profiles patch <id> --body-file - <<'EOF'
    { "name": "my-profile", "profile_type": "advanced", "profile_status": "deleted" }
    EOF
    ```

- **`dictionaries replace` may return 204** — region-dependent. CLI re-GETs on 204; if that fails, it prints `replaced <id> (state not echoed by region)`. Always `get --keywords` after replace to canonically observe state.
- **`dictionaries create/replace` are multipart** — `--file` is required for both. Metadata via flat flags or `--metadata-file`. Never set `Content-Type` manually.
- **Patterns `delete` is soft** — archived server-side, invisible to `list`, still resolvable via `get` with `status: "deleted"`.
- **Filtering profiles have no `create`** — provision new profiles in the Strata Cloud Manager UI, then manage them via CLI.

## See also

- [Filtering Profiles](filtering-profiles.md) · [Patterns](patterns.md) · [Profiles](profiles.md) · [Dictionaries](dictionaries.md)
- [Configuration Management](../config-management.md) — non-DLP runtime config CRUD
- [Environment Variables](../../reference/environment-variables.md)
