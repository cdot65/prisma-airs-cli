# CLI Docs — Live Examples Workflow

Every `airs` command must render at least one real input + real output on its
docs page. This directory drives that rendering.

**Doc pages under `docs/cli/runtime/`, `docs/cli/redteam/`, and
`docs/cli/model-security/` are AUTO-GENERATED. Never edit them by hand —
your edit will be overwritten by `pnpm docs:gen` and will fail
`pnpm docs:check` in CI.**

## How the pipeline works

1. `scripts/gen-cli-docs.ts` walks the live `airs` program tree, joins each
   command with examples from the YAML sidecars in this directory, and writes
   `docs/cli/<area>/<page>.md`.
2. `scripts/check-cli-docs.ts` (`pnpm docs:check`, runs in CI) regenerates
   into a tempdir, byte-compares against the committed `.md`, and fails on
   drift. It also fails when a leaf command has no examples AND is not on
   `.missing-allowlist`.
3. The allowlist is the burndown list — every entry should disappear as
   examples land, except for entries explicitly blocked by an upstream / CLI /
   SDK defect (see below).

## Files in this directory

```
docs/cli/examples/
├── README.md                  # this file
├── REDACTION.md               # redaction rules (apply before pasting output)
├── .missing-allowlist         # commands awaiting examples (burn to zero)
├── runtime.yaml               # sidecar for `airs runtime ...` commands
├── redteam.yaml               # sidecar for `airs redteam ...` commands
├── model-security.yaml        # sidecar for `airs model-security ...` commands
├── dlp/{dictionaries,patterns,profiles,filtering-profiles}/
├── runtime/                   # body-file fixtures for runtime/* write ops
├── redteam/                   # body-file fixtures for redteam/* write ops
└── model-security/            # body-file fixtures for model-security/* write ops
```

Sidecar files key by **full command path** (no `airs` prefix):

```yaml
"runtime dlp dictionaries list":
  examples:
    - note: Pretty output (default)
      input: airs runtime dlp dictionaries list
      output: |
        <live capture, redacted>
```

The renderer emits each entry as italic note + bash codefence (input) + text
codefence (output). There is no `!!! bug` admonition path — see "Blocked
commands" below for how to handle defects.

## Per-command authoring workflow

1. `cd ~/development/cdot65/prisma-airs-cli` (required — `.env` auto-loads
   `PANW_AI_SEC_API_KEY`, `PANW_MGMT_*`, `PANW_DLP_ENDPOINT`).
2. For any write op needing `--body-file`, drop a minimal valid fixture under
   `docs/cli/examples/<area>/<command>.json`. Redact per
   [REDACTION.md](REDACTION.md) before committing.
3. Live-capture each command:
   - **list / get** → run three times: pretty (default), `--output json`,
     `--output yaml`. One sidecar entry per format.
   - **create / update / replace / patch / delete** → JSON only.
4. Redact every capture per [REDACTION.md](REDACTION.md).
5. Append entries to the right sidecar (`runtime.yaml` / `redteam.yaml` /
   `model-security.yaml`).
6. Remove the command path from `.missing-allowlist`.
7. `pnpm docs:gen` — regenerates `docs/cli/**/*.md`.
8. `pnpm format` — Biome fixes JSON fixture formatting.
9. `pnpm docs:check` — must pass before commit.
10. Add `.changeset/<slug>.md` (`patch`) describing what got examples.
11. Open PR.

## Format contract (sidecar entries per command)

**list / get** — three entries (pretty / JSON / YAML):

```yaml
"runtime dlp dictionaries list":
  examples:
    - note: Pretty output (default)
      input: airs runtime dlp dictionaries list
      output: |
        ID                                    NAME                CATEGORY
        00000000-0000-0000-0000-000000000001  example-dict        custom
    - note: JSON output
      input: airs runtime dlp dictionaries list --output json
      output: |
        [
          {
            "id": "00000000-0000-0000-0000-000000000001",
            "name": "example-dict",
            "category": "custom"
          }
        ]
    - note: YAML output
      input: airs runtime dlp dictionaries list --output yaml
      output: |
        - id: 00000000-0000-0000-0000-000000000001
          name: example-dict
          category: custom
```

**create / update / replace / patch / delete** — JSON only, plus a
reference to the fixture if one is used:

```yaml
"runtime dlp patterns create":
  examples:
    - note: Create from body fixture (see docs/cli/examples/dlp/patterns/create.json)
      input: airs runtime dlp patterns create --body-file docs/cli/examples/dlp/patterns/create.json --output json
      output: |
        {
          "id": "00000000-0000-0000-0000-000000000001",
          "name": "example-pattern",
          "state": "created"
        }
```

## Blocked commands (upstream / CLI / SDK defect)

The renderer has no `!!! bug` admonition path — every sidecar entry produces
input + output codefences. So when live capture is impossible:

1. **File a tracking issue first.** Upstream API → label `upstream-bug` in
   `cdot65/prisma-airs-cli`. CLI defect → label `bug` in
   `cdot65/prisma-airs-cli`. SDK defect → label `bug` in
   `cdot65/prisma-airs-sdk`. Search existing issues first
   (#80 already covers DLP `patterns get` / `profiles get` 400).
2. **Leave the entry on `.missing-allowlist`** with a comment line above
   linking the tracking issue:

   ```
   # Blocked by upstream — see https://github.com/cdot65/prisma-airs-cli/issues/80
   runtime dlp patterns get
   ```

3. The doc page will render the default `!!! warning "Example needed"`
   placeholder. That is correct until the bug is fixed — better than
   fabricating output.
4. Reference the tracking issue from your changeset so reviewers know why
   the entry survives the burndown.

## Capture environment

All `airs` invocations during capture **must** run from
`~/development/cdot65/prisma-airs-cli`. The repo-local `.env` (and
`.env.cdot65` / `.env.dus` for alternate tenants) supplies the management +
runtime credentials. Running from anywhere else fails with
"missing credentials".

## Checklist before opening the PR

- [ ] Sidecar entries added; YAML loads cleanly (try `pnpm docs:gen`).
- [ ] `.missing-allowlist` updated (entries removed for completed commands;
      comment + entry kept for blocked commands with tracking issue link).
- [ ] Body fixtures redacted per REDACTION.md.
- [ ] `pnpm docs:gen` run; regenerated `.md` committed.
- [ ] `pnpm format` clean.
- [ ] `pnpm docs:check` clean.
- [ ] `.changeset/<slug>.md` present (`patch`).
- [ ] No edits to `.md` files outside the autogenerated set.
