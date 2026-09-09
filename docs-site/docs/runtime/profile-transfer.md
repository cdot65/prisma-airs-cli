---
title: Profile backup and tenant migration
---

# Profile backup and tenant migration

Export Runtime security profile configuration to private JSON or YAML files, then restore
it into the selected tenant using the SDK's Management OAuth and resource clients.
This is configuration migration, not a traffic-log backup or HTML environment report.

For copy-and-paste commands and actual full migration evidence, see the
[full tenant migration workflow](./profile-migration-workflow.md).

For the validated three-profile example that **preserves custom DLP**, see
[prod to dev with custom DLP](./prod-dev-migration.md). It includes operator receipts,
independent policy/read-back checks and saved shell helpers for recovery.

## Back up

```bash
airs runtime profiles backup --all --output-file ./profiles.json
airs runtime profiles backup "Production" --file-format yaml --output-file ./production.yaml
```

Without a selector, all latest profiles are exported. A selector matches an exact profile
name or ID. The version-1 envelope contains the source TSG, export time, latest profile
policies, and the exact referenced custom-topic definitions. Historical profile revisions
are not exported. Unknown policy settings are preserved rather than silently discarded.

The default filename is unique and created in the **current working directory**, not
`~/.prisma-airs`. Existing files and symlink destinations are never overwritten. Backups
use mode `0600` on POSIX systems and are limited to 20 MiB. They contain sensitive policy
configuration and topic examples; store/transport them securely. They do not include
CLI credentials. Raw backups cannot be sent to stdout; `--output` selects only the
operation-summary format. Use `--file-format json|yaml` for the file's encoding.

`--max-pages` defaults to 100 (100 items/page, at most 10,000 records per inventory).
Incomplete or nonadvancing pagination fails instead of producing a partial backup.

## Switch and preview

```bash
airs tenant create source --config /secure/source.json
airs tenant create destination --config /secure/destination.json
airs tenant switch source
airs runtime profiles backup --all --output-file ./profiles.json
airs tenant switch destination
airs runtime profiles restore ./profiles.json --dry-run --output json
```

The dry run authenticates and reads destination inventories, but creates/updates nothing.
It validates the complete input and dependency plan. All profile names must be unique
in the backup. Missing policies, ambiguous identities, unavailable topic revisions,
unrecognized tenant-bound ID fields, and incomplete inventories fail safely.

## Restore

```bash
# Interactive confirmation identifies the source and destination TSGs.
airs runtime profiles restore ./profiles.json

# Noninteractive restore requires an explicit destination assertion.
airs runtime profiles restore ./profiles.json --expect-tsg 200 --force --output json

# Keep existing profiles untouched, or explicitly update them.
airs runtime profiles restore ./profiles.json --on-conflict skip --dry-run
airs runtime profiles restore ./profiles.json --on-conflict update --expect-tsg 200 --force

# Recover a partial restore: verify existing profiles, create only missing profiles.
airs runtime profiles restore ./profiles.json --on-conflict verify --on-missing-dlp basic --dry-run

# Create separately named profiles and topics.
airs runtime profiles restore ./profiles.json --name-prefix imported- --dry-run
```

Replace `200` with the destination TSG. A mismatched `--expect-tsg` fails before any API
request. `--force` only skips confirmation; it never overrides name conflicts or validation.
Existing profile names fail by default; `skip` leaves them and their dependencies alone;
`update` deliberately writes a new destination profile revision. `--name-prefix` applies
to profile and custom-topic names.

The `verify` conflict mode (CLI 5.7.0+) is a safe recovery option: resolve the source's topic
and DLP bindings into destination identities, then compare existing profiles with that
effective policy and active state. A mismatch or missing dependency fails the entire plan
before any writes. Matching profiles appear as `verify` in previews and `verified` in results;
they are never updated. Missing profiles are still created after confirmation. Identities,
revisions and policies are rechecked during execution to detect concurrent changes.

Server-assigned profile IDs, source TSG/CSP IDs, revisions, and audit metadata are not
replayed. The destination assigns identities. Topic references are rewritten to verified
destination IDs and revisions. An existing topic is reused only if its definition and
active state match; shared topics are never updated automatically. Differing definitions
require a prefix. A backup containing multiple revisions of the same topic name must be
split by profile/revision before restore; the CLI will not choose one arbitrarily.

### DLP dependencies

Runtime's built-in **Basic** detection (`sensitive content`, empty ID, version `2`)
is portable and needs no Enterprise DLP mapping. Custom/Advanced DLP data profiles
must already exist at the destination to preserve their bindings. Map each custom source
name explicitly (repeat the flag for multiple dependencies):

```bash
airs runtime profiles restore ./profiles.json --dry-run \
  --dlp-map 'Source PII=Destination PII' \
  --dlp-map 'Source Financial=Destination Financial'
```

The CLI resolves and verifies destination IDs/versions and preserves policy actions.
It does **not** clone DLP profiles, patterns, dictionaries, or other shared DLP resources.
Review destination DLP definitions yourself: matching names or versions do not prove
equivalent detection behavior. Same-tenant restores retain their existing DLP bindings.

#### Explicit Basic fallback (MVP; CLI 5.7.0+)

Use `--on-missing-dlp basic` to accept Basic detection for unresolved custom dependencies.
The default remains `error`, preventing an unattended protection downgrade.

```bash
airs runtime profiles restore ./profiles.json --on-missing-dlp basic --dry-run --output json

# After reviewing every protection change, restore with confirmation.
airs runtime profiles restore ./profiles.json --on-missing-dlp basic --expect-tsg 200

# Preserve a known destination binding; other unresolved dependencies may fall back.
airs runtime profiles restore ./profiles.json --on-missing-dlp basic \
  --dlp-map 'Source PII=Destination PII' --dry-run
```

Explicit mappings are resolved first. Unmapped cross-tenant references are deliberately
abandoned with this flag; the CLI does not claim they are absent or query the Enterprise DLP
catalog just to discard them. Mapped targets, and same-tenant references when this flag is
used, are checked against a complete destination inventory. Authentication, permission,
network, ambiguous-identity, unknown-version, and incomplete-inventory errors still fail.

If **any** custom dependency in a profile is unresolved, that profile's complete custom DLP
configuration becomes Basic, including other mapped custom rules in the same profile.
The plan and result expose `dlpFallbacks`: `unresolved` identifies the reason and `replaced`
lists **all** discarded custom dependencies. Warnings go to stderr even with `--quiet`;
JSON/YAML stdout remains parseable. Skipped profiles and their dependencies stay untouched.

Block/allow actions and masking settings are preserved. Disabled DLP stays disabled.
Fallback refuses inline masking without a block action or an unsupported action, rather
than changing enforcement silently. Custom patterns, exceptions, thresholds and dictionaries
are **not** reproduced by Basic. Keep the original backup to restore Advanced settings later.

To investigate dependencies before retrying, use `airs runtime dlp profiles list --all --output json`
and `airs runtime dlp profiles get <id> --output json` in each tenant, plus the `patterns`
and `dictionaries` commands for their referenced resources. These inspection/CRUD commands
are not a dependency-complete DLP backup/restore workflow. Automatic recursive migration
of those shared resources remains future work.

See [MVP acceptance results](./dlp-fallback-validation.md) for live cross-tenant validation.

### Failure and concurrency behavior

Restore rechecks profile/topic/DLP identities after planning and profiles immediately
before each write, then reads back each profile to verify its policy and active state.
The API does not provide an atomic multi-resource transaction or compare-and-swap here;
avoid concurrent administration of the same resources. An immediate read-back mismatch
fails verification, including when the service has not yet made a write visible.

Verification permits only specific observed server additions when the source omitted the
field: database/source-code/malicious-code/URL/prompt-injection/agent severity values,
the observed toxicity confidence-severity pair (detector-level or per category under a
matching toxicity detector), a null database-security section and an
empty default URL category (`member: null`). These accepted fields are listed under
`serverDefaults` in JSON/YAML output. Explicit source values, changed actions, unknown
additions and other policy differences still fail. No source policy is rewritten to hide
differences, and this comparison rule does not change SDK parsing or claim a spec update.

If a later step fails, completed writes remain and the result reports them with
`complete: false` and a nonzero exit. There is no automatic rollback or write retry.
Inspect the destination before retrying, particularly after a timeout whose commit
status may be unknown. Do not blindly rerun with `--on-conflict update`.

For the stopped 19-profile migration, a verified dry run now confirms one existing profile
and 26 matching topics can be retained; 18 profiles remain to create, including three Basic
DLP fallbacks. The full migration has **not** been resumed automatically. Use `--on-conflict
verify` to check existing resources rather than skipping their verification.

Human `pretty`, `table` and `markdown` output lists one resource per row with separate
fallback and server-default sections. JSON/YAML keep complete structured records and IDs;
CSV retains the single summary record with nested JSON fields.

`--debug` and enabled `PANW_AI_SEC_DEBUG` are rejected for transfer commands, preventing
sensitive policy bodies from being persisted in debug logs. Invalid JSON/YAML and schema
errors do not echo file contents. JSON/YAML output is a single-element summary array.

## Validated CLI output — 2026-09-08

Verified again at **19:42 UTC using the npm-installed CLI 5.5.0**, with SDK 0.29.0.
The installed package's seven files match the independent release build byte-for-byte.

Actual live backup summary (private artifact directory and TSG replaced with placeholders):

```bash
airs runtime profiles backup --all --output-file ./all-profiles.json --output json
```

```json
[
  {
    "file": "<working-directory>/all-profiles.json",
    "sourceTsgId": "<source-tsg>",
    "profiles": 19,
    "topics": 26
  }
]
```

Live acceptance authenticated using the read-only config, exported all 19 profiles and
26 referenced topics, and dry-ran a restore that skipped all 19 existing profiles.
It then created a synthetic profile/topic, exported JSON and YAML, restored a separately
named copy, and verified the new profile's exact policy and rewritten topic identity.
Collision/no-overwrite checks passed. Only the two synthetic profiles and two synthetic
topics were deleted afterward; their absence and the unchanged config hash were verified.

Actual synthetic restore stdout (TSG redacted; these test resources were subsequently deleted):

```json
[
  {
    "sourceTsgId": "<tenant-tsg>",
    "destinationTsgId": "<tenant-tsg>",
    "complete": true,
    "topics": [
      {
        "name": "copy-6aba28f4-cli-transfer-e2e-6aba28f4",
        "id": "0567af6e-5327-46ca-a40d-5589a33b599c",
        "action": "created"
      }
    ],
    "profiles": [
      {
        "name": "copy-6aba28f4-cli-transfer-e2e-6aba28f4",
        "action": "created",
        "id": "08b36bc7-ca94-491c-884b-400117087f81"
      }
    ]
  }
]
```

The real CLI + SDK OAuth integration test also runs against two isolated HTTP tenants:
source TSG `100`, destination TSG `200`. It proves credential selection, read-only config
preservation, dry-run behavior, and fresh destination profile/topic IDs. **Live testing
used one configured cloud tenant in this earlier run.** The subsequent Basic DLP MVP
acceptance linked above uses a synthetic cross-tenant backup and a second live destination.

Reproduce the isolated test with `pnpm exec vitest run tests/integration/tenant-profile-transfer.spec.ts`.
After building, run `node scripts/e2e-tenant-profiles.mjs` for live acceptance. That script
creates only uniquely named synthetic resources, removes them, and saves private evidence
under `artifacts/tenant-profiles-e2e-*`; it never changes your normal tenant selection.
