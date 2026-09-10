---
title: DLP backup and restore
---

# DLP backup and restore

`airs runtime dlp backup` and `airs runtime dlp restore` move custom DLP configuration
between tenants through a private file, following the same transfer contract as
`airs runtime profiles backup`/`restore`: complete-inventory export, plan-before-mutate,
re-check after confirmation, read-back verification of every write, and honest partial
completion. No update or delete is ever issued — the DLP profile write path is excluded
until it is live-verified.

## Backup

```bash
airs runtime dlp backup --output-file ./dlp-backup.json
airs runtime dlp backup --resources profiles --skip-unsupported \
  --output-file ./profiles.yaml --file-format yaml
```

- Exports **custom** dictionaries (with their keyword payloads), custom and
  file-property data patterns, and custom data profiles. Predefined (PANW-shipped)
  resources are never exported as content; when a profile references one, the exact
  referenced record is embedded as a **resolve-only reference** so restore can rebind
  it in the destination catalog by name.
- Profile dependency closure is captured at the exact referenced revision: a rule leaf
  pinning a pattern version the catalog no longer holds fails the export (or is
  excluded with a reason under `--skip-unsupported`, as are multi-profile rules and
  direct EDM dataset references). All unsupported profiles are reported in a single
  failure, so one review covers them all.
- Backups contain dictionary keywords: they are refused on stdout, written atomically
  at mode 0600 without overwriting, and capped at 20 MiB.

## Restore

```bash
airs runtime dlp restore ./dlp-backup.json --dry-run --output json
airs runtime dlp restore ./dlp-backup.json --pattern-map "EDM SSN=Dest SSN" --dry-run
airs runtime dlp restore ./dlp-backup.json --name-prefix migrated- \
  --expect-tsg <destination-tsg> --force
```

Restore stages **dictionaries → patterns → profiles**, threading source-ID →
destination-ID bindings forward and rewriting expression-tree leaves (id, name, and
version) to destination identities. The entire plan is validated before any write, and
destination state is re-checked after the confirmation prompt.

- Predefined references resolve against the destination catalog and error if absent.
- Patterns using tenant-bound techniques (EDM, fingerprints, trained models, linked
  dictionaries) are never recreated: bind them to pre-provisioned destination patterns
  with `--pattern-map "source-name=destination-name"`.
- `--on-conflict` for data profiles is `error` (default), `verify` (read-only resume),
  or `skip`. There is no `update`.
- Every create is verified by re-reading the record; a retired or divergent read-back
  stops the run, which reports exactly what completed and exits 1.

## End-to-end example: prod to dev

This walkthrough migrates a tenant's custom DLP configuration — a keyword dictionary
(`Compliance Keywords`), a custom regex pattern (`Customer Account Regex`), an EDM
pattern (`EDM Customer Records`), and a data profile (`PII Guardrail`) that references
both custom resources plus the predefined `Social Security Numbers` pattern — from a
production tenant into a development tenant. TSG IDs below are placeholders.

### 1. Register both tenants once

```bash
airs tenant create prod --config /secure/prod.json
airs tenant create dev --config /secure/dev.json
```

Both tenants use the standard Management OAuth credentials; DLP needs no extra
configuration (`PANW_DLP_ENDPOINT` is an optional endpoint override only).

### 2. Back up from the source tenant

```bash
airs tenant switch prod
airs runtime dlp backup --output-file ./dlp-backup.json
```

The operation summary reports the file path, source TSG, and per-resource counts. The
envelope embeds the dictionary's keywords (required to recreate it), so treat the file
as sensitive: it is created privately in the current directory at mode `0600`, never
overwrites, and is refused on stdout. Counts can be confirmed without opening keyword
content:

```bash
jq '{dictionaries: (.dictionaries | length), patterns: (.patterns | length), profiles: (.profiles | length)}' ./dlp-backup.json
```

#### When the export refuses: unsupported profiles

A tenant often carries data profiles the transfer cannot rebuild faithfully — most
commonly profiles built on a **multi-profile rule** (a rule that references *other data
profiles* by server-assigned numeric id, such as SDK example profiles), and profiles
referencing an EDM dataset directly. The export refuses these by default, lists every
offender in one failure, and names the remedy:

```
✗ Profiles cannot be exported: sdk-example-5950c4ed (Unsupported detection rule
  type: multi_profile); exclude unsupported profiles explicitly with --skip-unsupported
```

Failing closed is deliberate: the backup file does not record what was left out, so a
partial backup must be one explicit operator decision at export time — never something
a later restore quietly inherits. Once you have reviewed the list, re-run with the
exclusion made explicit; each excluded profile is warned individually and counted in
the operation summary:

```bash
airs runtime dlp backup --skip-unsupported --output-file ./dlp-backup.json
```

Multi-profile rules cannot transfer because their numeric profile ids are assigned per
tenant by the server; transplanting or guessing a remapping would change which
detection actually applies. Runtime AI security profiles bind DLP through individual
data profile references, so excluding a multi-profile data profile does not affect the
Runtime profiles this CLI migrates — but if you need the multi-profile definition in
the destination, recreate it there against the destination's own profile ids.

### 3. Preview the restore against the destination

```bash
airs tenant switch dev
airs runtime dlp restore ./dlp-backup.json --dry-run --output json
```

The first preview fails, because the EDM pattern is bound to a tenant-side dataset a
backup cannot recreate:

```
Data pattern uses a tenant-bound detection technique: EDM Customer Records;
provision it in the destination and bind it with
--pattern-map "EDM Customer Records=<destination-name>"
```

Create (or identify) the equivalent EDM pattern in the destination tenant first — EDM
datasets are provisioned per tenant outside this CLI — then bind it and preview again:

```bash
airs runtime dlp restore ./dlp-backup.json --dry-run --output json \
  --pattern-map "EDM Customer Records=Dev EDM Customer Records"
```

```json
[
  {
    "sourceTsgId": "1010101010",
    "destinationTsgId": "2020202020",
    "dryRun": true,
    "dictionaries": [
      { "name": "Compliance Keywords", "action": "create" }
    ],
    "patterns": [
      { "name": "Customer Account Regex", "action": "create" },
      { "name": "Dev EDM Customer Records", "action": "map" },
      { "name": "Social Security Numbers", "action": "resolve" }
    ],
    "profiles": [
      { "name": "PII Guardrail", "action": "create" }
    ]
  }
]
```

`create` means the resource does not exist in the destination and will be created;
`map` binds through your explicit `--pattern-map`; `resolve` matched a predefined
resource in the destination catalog by name — predefined content is never created. The
dry run authenticates and reads destination inventories but writes nothing.

### 4. Execute

```bash
airs runtime dlp restore ./dlp-backup.json \
  --pattern-map "EDM Customer Records=Dev EDM Customer Records"
```

The interactive confirmation names both TSGs and the number of writes. For scripts,
assert the destination explicitly instead of prompting:

```bash
airs runtime dlp restore ./dlp-backup.json \
  --pattern-map "EDM Customer Records=Dev EDM Customer Records" \
  --expect-tsg 2020202020 --force
```

Execution re-checks destination state after confirmation, then writes in stage order —
the dictionary, then the pattern, then the profile, rewriting the profile's rule leaves
to the destination ids and versions as it goes. Every create is re-read and verified;
the summary lists each resource as `created`, `mapped`, `resolved`, or `reused`.

### 5. Verify, resume, and recover

Re-running the same restore is safe: identical dependencies are reused without writes,
and `--on-conflict verify` turns the profile stage into a read-only check that the
destination matches the backup:

```bash
airs runtime dlp restore ./dlp-backup.json \
  --pattern-map "EDM Customer Records=Dev EDM Customer Records" \
  --on-conflict verify
```

If a run stops partway — an API failure, a race with another operator — nothing is
rolled back: the summary reports exactly which writes completed, the command exits 1,
and the resume above verifies completed resources and creates only what is missing. A
destination that drifted from the plan fails with `changed after planning` before any
write. To restore alongside existing resources rather than into them, prefix everything:

```bash
airs runtime dlp restore ./dlp-backup.json --name-prefix migrated- \
  --pattern-map "EDM Customer Records=Dev EDM Customer Records"
```

## Review evidence

Reviewed under the AIRS Transfer Contract's agent review protocol (gate: score ≥ 9/10).

- **Round 1 (2026-09-10): 4/10 — REVISIT.** Five findings, all addressed: unformatted
  spec (biome red); dependency closure did not enforce leaf version pins against the
  catalog (I-4); created records' lifecycle state was swallowed by the verification
  comparator (I-8); the predefined-dictionary resolve path and the verify-mismatch
  path were untested (P-1, I-9).
- **Round 2 (2026-09-10, full re-review): 9/10 — PASS.** All invariants I-1…I-10 and
  problems P-1…P-4 verified upheld with file:line evidence; checks green
  (biome, tsc, 1741 unit tests, module coverage 97.78% statements / 80.05% branches /
  100% functions). Accepted risks recorded in the review: the Spring short-page
  fallback mirrors the reference implementation; `dictionary`-technique patterns are
  map-only pending live schema evidence for the link key; the echo comparator reports
  rather than fails unrequested server additions until live evidence can seed an
  allowlist; and no live DLP create evidence exists yet — a recorded live acceptance
  run is the path to 10/10 and to any future `--on-conflict update`.
