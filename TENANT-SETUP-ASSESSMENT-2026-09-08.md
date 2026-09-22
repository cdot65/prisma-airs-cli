# Guided tenant configuration assessment — 2026-09-08

## Delivered

- `airs tenant create <name>` prompts for TSG ID, client ID, and a hidden secret.
- Existing-file `--config` registration remains supported without changing that file.
- `airs tenant set <name> <key> [value]` edits individual schema-backed settings.
- Hidden prompts and stdin support keep secret values out of command arguments/output.
- Private configs, atomic replacement, per-config locks, pinned identity, read-only
  checks, and unrelated-field preservation protect the existing tenant behavior.
- Cancellation before saving leaves no partial registration or config. Config files
  remain on disk when a tenant is unregistered, including CLI-created files.

## Validation

| Check | Result |
| --- | --- |
| Full regression suite | 1,619 passed, 114 files; 34 added tests |
| Built CLI setup integration | 4 passed: local OAuth/profile list, per-field edits, stdin/rotation, rejected inputs |
| Built CLI profile-transfer regression | 2 passed: isolated source/destination migration and safety refusals |
| Actual terminal interaction | Guided setup, hidden secret entry, prompted setting update, and Ctrl+C verified |
| Cancellation artifact check | Only completed registration/config retained; active selection unchanged; exit 130 |
| TypeScript and bundled build | Passed |
| Release workflow tests | 14 passed |
| Lint | Passed; seven pre-existing warnings in `runtime-commands.spec.ts` |
| Docusaurus production build | Passed; tenant page includes sanitized actual terminal output |

Private synthetic-only terminal artifacts are in `/var/tmp/airs-guided-setup-SM5vRy`.
Cloud secrets/configuration were not changed. The OAuth integration uses a local HTTP
server and the actual CLI/SDK: it is not a live-cloud authentication claim.

## Assessment and boundaries

Self-assessment: 9/10 for the requested local configuration feature. Successful save
does not assert that OAuth credentials work; use an API command to verify access.
Secrets are private-file plaintext, not OS-keychain encrypted. Abandoned locks require
manual investigation, and edits require writable files/directories. These constraints
are documented. Broader keychain integration or automatic stale-lock recovery is not
part of this request.

## Published release verification

- CLI **5.6.0** is published as npm `latest`, with GitHub provenance, and installed
  globally. `airs --version` returns `5.6.0`. SDK dependency remains **0.29.0**.
- Release source: `42f70e085e46c872902605e703c3f7f1b16038b9`, pushed to GitHub and
  the origin mirror along with annotated tag `v5.6.0`.
- All seven installed package files exactly match the independently packed and tested
  candidate. Tarball SHA-256:
  `958a7df53f7ef2a0470d03de483e2c8140948f1436b08c273b8dd7ef44dff720`.
- npm-installed acceptance at 2026-09-08T21:47Z: six tenant setup/OAuth/migration
  workflows and eleven native consumer checks passed; native fixture cleanup completed.
- Coverage: 97.27% lines/statements, 89.91% branches, 96.47% functions under the
  repository coverage configuration (which excludes CLI rendering/command files).
  Tenant settings storage achieved 100% lines/functions and 93.93% branches.
- Production tenant documentation returned HTTP 200 and contained guided setup,
  stdin instructions, and captured terminal output.

Release evidence: [v5.6.0](https://github.com/cdot65/prisma-airs-cli/releases/tag/v5.6.0),
[CI](https://github.com/cdot65/prisma-airs-cli/actions/runs/34282028905),
[npm publication](https://github.com/cdot65/prisma-airs-cli/actions/runs/34282152684),
[docs deployment](https://github.com/cdot65/prisma-airs-cli/actions/runs/34282028896),
[container workflow](https://github.com/cdot65/prisma-airs-cli/actions/runs/34282150621).
