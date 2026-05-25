# CLI Docs — Live Examples Template

This directory holds the fixture and example conventions used by every page
under `docs/cli/runtime/`, `docs/cli/redteam/`, and `docs/cli/model-security/`.

Project rule: **every command must have a real input + real output**. No
fabricated payloads. When a live capture is blocked by an upstream API, CLI,
or SDK defect, document it with a `!!! bug` admonition and a tracking-issue
link instead of inventing output.

## Capture environment

All `airs` invocations during capture **must** run from
`~/development/cdot65/prisma-airs-cli`. The repo-local `.env` (or
`.env.cdot65` / `.env.dus` for alternate tenants) supplies
`PANW_AI_SEC_API_KEY`, `PANW_MGMT_*`, and `PANW_DLP_ENDPOINT`. Running from
anywhere else fails with missing-credential errors.

## Directory layout

```
docs/cli/examples/
├── README.md                  # this file
├── REDACTION.md               # redaction rules
├── dlp/
│   ├── dictionaries/          # fixtures for write ops on dlp dictionaries
│   ├── patterns/
│   ├── profiles/
│   └── filtering-profiles/
├── runtime/                   # fixtures for runtime/* write ops
├── redteam/                   # fixtures for redteam/* write ops
└── model-security/            # fixtures for model-security/* write ops
```

Fixture files live under `docs/cli/examples/<area>/<command>.json` and follow
the redaction rules in [REDACTION.md](REDACTION.md).

## Example block format (the contract)

Replaces each `!!! warning "Example needed"` placeholder. For **list/get**
commands, show Pretty + JSON + YAML:

````markdown
#### Examples

**Input**
```bash
airs runtime dlp dictionaries list
```

**Output (pretty / default)**
```text
<live capture, redacted>
```

**JSON**
```bash
airs runtime dlp dictionaries list --output json
```
```json
<live capture, redacted>
```

**YAML**
```bash
airs runtime dlp dictionaries list --output yaml
```
```yaml
<live capture, redacted>
```
````

For **write ops** (`create` / `update` / `replace` / `patch` / `delete`),
only the JSON capture appears (plus the body/fixture if applicable).

## Bug admonition variant

When live capture is blocked by an upstream API, CLI, or SDK defect, replace
the example block with a `!!! bug` admonition that links the tracking issue.
Never fabricate output.

````markdown
#### Examples

**Input**
```bash
airs runtime dlp patterns get <id>
```

!!! bug "Upstream API returns 400"
    The DLP `/v2/api/data-patterns/{id}` endpoint currently returns HTTP 400 for all
    pattern UUIDs. Tracking: [cdot65/prisma-airs-cli#80](https://github.com/cdot65/prisma-airs-cli/issues/80).
    Example will be backfilled once the upstream API is fixed.

**Workaround**
```bash
airs runtime dlp patterns list --output json | jq '.[] | select(.id=="...")'
```
````

If no tracking issue exists yet, file one before adding the admonition:

- Upstream API defect → label `upstream-bug`
- CLI defect → label `bug` (this repo)
- SDK defect → label `bug` in `cdot65/prisma-airs-sdk`

## Per-page workflow

1. List every command on the page; confirm the placeholder is present.
2. Build fixtures under `docs/cli/examples/<area>/` for any write op needing
   `--body-file`.
3. Live capture: run each command up to three times (pretty / JSON / YAML)
   from `~/development/cdot65/prisma-airs-cli`; save outputs locally.
4. Redact per [REDACTION.md](REDACTION.md).
5. Detect bugs; file tracking issue + add `!!! bug` admonition if needed.
6. Swap the placeholder for the example block above.
7. Add a `.changeset/<slug>.md` (`patch`) with user-facing description.
8. Open the PR; post the same summary as a PR comment.
