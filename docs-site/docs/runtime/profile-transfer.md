---
title: Profile backup and tenant migration
---

# Profile backup and tenant migration

Export Runtime security profile configuration to private JSON or YAML files, then restore
it into the selected tenant using the SDK's Management OAuth and resource clients.
This is configuration migration, not a traffic-log backup or HTML environment report.

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

# Create separately named profiles and topics.
airs runtime profiles restore ./profiles.json --name-prefix imported- --dry-run
```

Replace `200` with the destination TSG. A mismatched `--expect-tsg` fails before any API
request. `--force` only skips confirmation; it never overrides name conflicts or validation.
Existing profile names fail by default; `skip` leaves them and their dependencies alone;
`update` deliberately writes a new destination profile revision. `--name-prefix` applies
to profile and custom-topic names.

Server-assigned profile IDs, source TSG/CSP IDs, revisions, and audit metadata are not
replayed. The destination assigns identities. Topic references are rewritten to verified
destination IDs and revisions. An existing topic is reused only if its definition and
active state match; shared topics are never updated automatically. Differing definitions
require a prefix. A backup containing multiple revisions of the same topic name must be
split by profile/revision before restore; the CLI will not choose one arbitrarily.

### DLP dependencies

Cross-tenant DLP data profiles must already exist at the destination. Map every source
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

### Failure and concurrency behavior

Restore rechecks profile/topic/DLP identities after planning and profiles immediately
before each write, then reads back each profile to verify its policy and active state.
The API does not provide an atomic multi-resource transaction or compare-and-swap here;
avoid concurrent administration of the same resources. An immediate read-back mismatch
fails verification, including when the service has not yet made a write visible.

If a later step fails, completed writes remain and the result reports them with
`complete: false` and a nonzero exit. There is no automatic rollback or write retry.
Inspect the destination before retrying, particularly after a timeout whose commit
status may be unknown. Do not blindly rerun with `--on-conflict update`.

`--debug` and enabled `PANW_AI_SEC_DEBUG` are rejected for transfer commands, preventing
sensitive policy bodies from being persisted in debug logs. Invalid JSON/YAML and schema
errors do not echo file contents. JSON/YAML output is a single-element summary array.

## Validated CLI output — 2026-09-08

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
        "name": "copy-b0070cb2-cli-transfer-e2e-b0070cb2",
        "id": "916c6877-dbb7-47cf-8fe1-8ac61bdad42f",
        "action": "created"
      }
    ],
    "profiles": [
      {
        "name": "copy-b0070cb2-cli-transfer-e2e-b0070cb2",
        "action": "created",
        "id": "6c3a0d03-b18c-4fd0-9a2c-cabc9bfe6b5d"
      }
    ]
  }
]
```

The real CLI + SDK OAuth integration test also runs against two isolated HTTP tenants:
source TSG `100`, destination TSG `200`. It proves credential selection, read-only config
preservation, dry-run behavior, and fresh destination profile/topic IDs. **Live testing
used one configured cloud tenant; a two-cloud-tenant restore has not been claimed.**

Reproduce the isolated test with `pnpm exec vitest run tests/integration/tenant-profile-transfer.spec.ts`.
After building, run `node scripts/e2e-tenant-profiles.mjs` for live acceptance. That script
creates only uniquely named synthetic resources, removes them, and saves private evidence
under `artifacts/tenant-profiles-e2e-*`; it never changes your normal tenant selection.
