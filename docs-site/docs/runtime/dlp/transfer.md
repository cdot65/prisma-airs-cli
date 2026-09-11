---
title: DLP backup and restore
---

# DLP backup and restore

`airs runtime dlp backup` and `airs runtime dlp restore` move custom DLP configuration
between tenants through a private file, following the same transfer contract as
`airs runtime profiles backup`/`restore`: complete-inventory export, plan-before-mutate,
re-check after confirmation, read-back verification of every write, and honest partial
completion. No update or delete is ever issued. Dictionary, pattern, and profile
creates are live-verified; every transfer create still requires an exact re-read.

## Backup

```bash
airs runtime dlp backup --output-file ./dlp-backup.json
airs runtime dlp backup --resources profiles --skip-unsupported \
  --output-file ./profiles.yaml --file-format yaml
```

- Exports **custom** dictionaries (with their keyword payloads), custom and
  file-property data patterns, and custom data profiles. Predefined (PANW-shipped)
  resources are never exported as content: when a profile references one, the backup
  embeds only a **slim resolve-only stub** — id, name, type, and detection technique —
  so restore can rebind the reference in the destination catalog. No predefined
  definition ever enters a backup file.
- Inventories use an explicit name sort and validate unique IDs, page counts, and
  totals. The live API caps requested 100-record pages to 50 and returns snake_case
  pagination metadata; both forms are supported. Overlapping pages and count drift
  stop the operation instead of silently dropping resources.
- Profile dependency closure is captured at the exact referenced revision: a rule leaf
  pinning a pattern version the catalog no longer holds fails the export (or is
  excluded with a reason under `--skip-unsupported`, as are multi-profile rules,
  direct EDM dataset references, and profiles referencing a **retired** pattern —
  restoring those would resurrect archived configuration in the destination). All
  unsupported profiles are reported in a single failure, so one review covers them
  all.
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

- Predefined references resolve against the destination catalog by identity first —
  a destination predefined pattern with the same id and detection technique is the
  same PANW-shipped pattern even when catalogs name it differently — then by exact
  name. Predefined catalogs are not guaranteed to be uniform across tenants
  (provisioning and licensing differ): when neither resolves, every miss is reported
  at once with ranked same-technique candidates, and you bind equivalents explicitly
  with `--pattern-map "<source-name>=<destination-name>"` — the binding wins over
  resolution, and the destination pattern is still only referenced, never created.
  Name similarity is never bound automatically: a near-match is a suggestion for you,
  not a decision by the CLI.
- Patterns using tenant-bound techniques (EDM, fingerprints, trained models, linked
  dictionaries) are never recreated: bind them to pre-provisioned destination patterns
  with `--pattern-map "source-name=destination-name"`.
- `--on-conflict` for data profiles is `error` (default), `verify` (read-only resume),
  `skip`, or `reconcile`. There is no `update` — the DLP profile update endpoint is a
  live HTTP 500.
- `--on-conflict reconcile` implements two rules for a name that already exists in the
  destination:
  1. **Active duplicate** — the profile is verified against the source end-state
     (identical to `verify`). Because there is no update path, a divergent active
     profile still fails: the API cannot make it match.
  2. **Archived-name collision** — a create that returns HTTP 409 names an archived
     profile the API never lists (profiles cannot be deleted, only archived in the
     SCM UI, and archived names are never released). Reconcile retries once under a
     unique suffix (`<name truncated to fit>-<6 hex>`, within the 32-char limit),
     verifies the created record by read-back, and reports each rename.
  Caveat: reconcile is **complete-once, not idempotent** for archived-name
  collisions. A suffixed copy carries a new name, so the original name stays
  tombstoned; re-running reconcile creates *another* suffixed copy rather than
  reusing the first. Run it once, or clear the archived profiles in the SCM UI first.
  Profiles created under a fresh name (no tombstone) verify normally on re-run.

  Live-exercised (2026-09-11, prod TSG 1001464285 → dev TSG 1158365485): a profiles
  restore into a dev tenant carrying archived-name tombstones completed under
  `reconcile` — six archived-name collisions were each suffixed and read-back
  verified, one fresh name created normally, every rename reported. A second run
  produced a *second* set of suffixes for the same source names (e.g.
  `Acceptance - Codename Gua-95cfa7` and `…-f26fdc`), which is the documented
  non-idempotency, not a defect.
- `--skip-unresolved` turns unresolvable references into explicit skips: the reference
  and **every profile that depends on it** are excluded from the restore, each warned
  individually and reported in the plan, the summary, and `--output json`. This covers
  missing predefined patterns and dictionaries, unmapped tenant-bound patterns, and
  patterns whose detection content the API does not export — a UI copy of a predefined
  pattern reads back without its regexes and cannot be recreated (live-verified as an
  HTTP 400); bind it with `--pattern-map` or skip it. Profiles
  are always skipped whole — a detection leaf is never removed from a restored
  profile, because that would silently weaken what it detects. Failing closed remains
  the default.
- The default pretty output narrates progress: inventory reads, the
  post-confirmation recheck, and each resource as it lands (`created`, `reused`,
  `resolved`, `mapped`, `verified`, `skipped`, with counts). Machine formats such as
  `--output json` print no progress, keeping stdout parseable.
- Every create is verified by re-reading the record, including its exact returned ID.
  A retired, mismatched, or unreadable record stops the run with exit 1. Confirmed POST
  receipts awaiting verification remain in `unverifiedCreates` (kind, name, ID), separate
  from verified completions, so a failed GET cannot hide a resource that was created.

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

The same binding rescues a **predefined-catalog mismatch**. Tenants do not always
carry identical predefined catalogs. A renamed built-in usually resolves itself — the
CLI matches predefined references by identity (same id, same detection technique)
before falling back to the name. When a reference truly cannot be resolved, the plan
reports every miss at once, each with ranked same-technique candidates from the
destination catalog:

```
✗ Missing predefined destination data patterns: Internet - ipv4 (candidates:
  "Internet - IPv4"); bind each with --pattern-map "<source-name>=<destination-name>"
```

Review the candidates (or list the catalog yourself with
`airs runtime dlp patterns list --all --include-predefined --output json` — listings
hide predefined records by default), then add one
`--pattern-map "Internet - ipv4=<name in destination>"` per miss alongside any EDM
bindings. Candidates are suggestions only — the CLI never binds a near-match on its
own, because attaching a lookalike detector would silently change what the restored
profile detects.

When no equivalent exists in the destination and losing the dependent profiles is
acceptable, `--skip-unresolved` restores everything else: each unresolved reference
and each skipped profile is warned and reported, so the loss is explicit rather than
discovered later.

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

## Live acceptance (2026-09-11)

A complete live migration ran between registered tenants (source TSG 1852583913 →
destination TSG 1158365485). The **pattern and profile create paths are live-verified
with read-back verification**; the dictionary create path is not (see below).

- **Backup**: 4 patterns / 3 profiles exported (8.6 KB with slim predefined stubs;
  the first full-tenant envelope of 7/7 was 16 KB); 9 profiles excluded with reasons
  under `--skip-unsupported` (multi-profile rules, retired dependencies, basic
  profiles without exported rules).
- **Restore, final pass**: `Restore complete` — 2 patterns reused, 1 profile
  verified (read-only resume of an earlier partial run), 1 profile created and
  verified by re-read, 1 profile skipped under `--skip-unresolved` (its predefined
  pattern does not exist in the destination catalog), with server-added fields
  reported.
- **Fail-safe demonstrations along the way**, four distinct stops, all exit 1 with
  honest reporting: one HTTP 400 before any write landed (zero writes reported); one
  HTTP 400 after the pattern stage completed, with the completed pattern writes
  reported; and two read-back verification refusals — the second naming the created
  record's id. HTTP-status failures always sanitize to a generic status line by
  design; created ids appear only in verification-refusal messages. The root causes
  are now handled: UI copies of predefined patterns and basic profiles read back
  without their detection content (unexportable classes, gated with explicit
  remedies), and the server normalizes `supported_confidence_levels` on create
  (tolerated and reported, never silently accepted).
- **Idempotency**: re-running the same restore reuses and verifies without writes.

This supersedes the May 2026 records of HTTP 400s for `POST /v2/api/data-patterns`,
`POST /v2/api/data-profiles`, and get-by-id on both: all work live; those 400s were
request-content classes, now classified.

**Superseded dictionary diagnosis:** the earlier detail-free 400 probe matrix used
region codes such as `GLOBAL`, `us`, and `us-west-2`. On 2026-09-11 the operator
created `test` in the prod SCM UI; its metadata exposed the region display name
`United States`. The same CLI/SDK multipart encoding succeeds with that value, with
no classification or tag additions. The entitlement hypothesis was disproven for
prod. Successful dictionary restoration into dev subsequently disproved a general
creation block there too.

## Remediation acceptance (2026-09-11, prod → dev)

Source TSG **1001464285**, destination TSG **1158365485**. SDK **0.30.1** is published;
the CLI pins that registry package without a local link. Live acceptance used the
built CLI from a private directory without `.env`. Only creates and reads were used;
created acceptance records remain available for inspection.

### Dictionary wire format and fidelity

- SCM-created source `test`: id `6aa36a956d8a3561e5c044c7`, 19 keywords,
  region `United States`, category `Academic`. No keyword content is included here.
- Stock SDK 0.30.0 + CLI 5.9.0 created `AirsProbe20260911`
  (`6aa36af4c1b63c34ceb55648`) when the region alone changed to `United States`.
  `json` remained an application/json Blob; `file` remained the keyword upload.
  No manual Content-Type, query, classification, or tags changes were required.
- TXT input with three synthetic words read back with all three. CSV consumes the
  first row as a header: the same three lines without a header read back as two.
  A header plus three lines read back as three. Quoting ordinary CSV words retains
  the quote characters literally; an RFC 4180 encoder would change detection.
- Restore now reconstructs CSV with one header followed by the original keyword
  lines, and TXT without a header. It never normalizes keyword text. Unsupported
  new-upload formats, embedded line breaks, empty keyword sets/words, and CSV
  multi-column content fail preflight. Exact read-only reuse of legacy envelopes
  does not require re-encoding their files.
- Before the encoder fix, restoring with prefix `BeforeFix ` created dictionary
  `6aa36d50268c871df572d790`, then refused its read-back due to the lost first word.
  Exit 1 named that unverified ID and stopped subsequent stages. This deliberately
  retained record is not a successful migration and must not be used as one.
- After the fix, the original version-1 envelope restored **five dictionaries** and
  verified every keyword, metadata projection, and original filename. `test` became
  `6aa36d99b79656b9fa75945e` in dev with all 19 keywords. The other four verified IDs
  are `6aa36d93374cddd1b56ac133`, `6aa36d95934932fa38a5e66a`,
  `6aa36d96268c871df572d791`, and `6aa36d98268c871df572d792`.
- SDK exceptions expose safe RFC 7807 title/detail/field diagnostics, including
  `originalFileName: must not be blank`, plus structured `problem` metadata. Unknown
  text and rejected values are withheld. DLP `--debug` logs omit request and response
  bodies, including SDK body-debug overrides; method, URL, status, and timing remain. Dictionary API failures exit 1; malformed
  local input exits 2 without echoing JSON fragments. An unexplained 400 suggests
  checking the exact SCM region label and makes no entitlement assertion.

[The vendor's dictionary guide](https://docs.paloaltonetworks.com/enterprise-dlp/administration/configure-enterprise-dlp/data-dictionaries)
requires an Enterprise DLP entitlement and describes selecting a storage region.
It does not make a bare 400 an entitlement diagnosis.

### Catalog completeness and profile references

The unsorted prod listing returned 1,126 rows but only 581 distinct IDs; dev returned
1,129 rows but only 595 distinct IDs. With `sort=name,asc`, all rows are distinct.
Both tenants contain **1,121 predefined names**, with **no shared predefined IDs**.
`Passport - UK` exists in both. Earlier sparse-catalog observations were incomplete
pagination, not proof of absent detectors.

- `Airs Acceptance Profile 20260911` created in prod as `11995031` and restored into
  dev as `11995033`. Its custom `Acceptance - Employee ID` reference changed from
  `6aa367b458e04896823e0ab9` to `6aa36809e3efdd642c62608f`; predefined `Passport - UK`
  changed from `6a914eb5425a966e5beda163` to `6a914e720697e8181686c0b8` by exact name.
  Both leaf versions, confidence, match mode, thresholds, and boolean structure
  survived read-back comparison. `body.ai_onboarding` was reported as server-added.
- A second source profile (`11995032`) restored and verified as dev `11995034`.
  It is named `Airs Basic Refusal 20260911` because it was a **basic-write probe**:
  the API ignored its requested basic type and created an advanced profile. It is
  not evidence of basic-profile migration. CLI basic writes now refuse before
  authentication instead of silently producing another type.
- Export refused source `Airs Multi Refusal 20260911` (`11995033`, `multi_profile`)
  and `Airs Retired Refusal 20260911` (`11995034`, archived pattern dependency),
  aggregating both reasons. `--skip-unsupported` excluded whole profiles explicitly.
- Dictionary rule leaves return nominal `version: 1` even when the request omitted
  it and dictionary GET exposes no version. Observed in prod profile `11995035` and
  dev profile `11995035`. Transfer preserves exactly this value through a verified
  dictionary binding; other pins still fail before any dependency create. Old
  envelopes omitting the version remain valid; server additions remain reported.
- The live profile-name boundary is 32 characters (`11995036` in dev); the otherwise
  identical 33-character request returned 400. Transfer enforces the boundary during
  create planning, including prefixed names, while exact legacy verification remains
  read-only. Names are not silently truncated.
- Both original dictionary classification tags (`pab`, `endpoint`) survived the
  observed round trips. Explicit source tags are now sent and compared; a changed
  classification fails verification. Response-only attributes were null in all
  observed dictionaries; no attribute-content fidelity is claimed.
- The final staged pass exported four supported profiles with one dictionary and
  three pattern dependencies, excluding the multi-profile and retired-dependency
  probes. In dev it reused the dictionary and an existing custom pattern, created
  `AirsStageRegex20260911` as `6aa37073934932fa38a5e66b`, and resolved `Passport - UK`.
  It verified the two existing profiles, created and verified `Airs Dictionary Profile
  20260911` as `11995037`, and created and verified `AirsStageProfile20260911` as
  `11995038`. Dictionary source ID `6aa36b6fc1b63c34ceb55649` remapped to destination
  `6aa36d98268c871df572d792` with nominal version 1 preserved. No unverified creates
  remained. A second pass reused all dependencies and verified all four profiles,
  with no creates, updates, or deletes.
- There are no truly missing predefined names in the current complete prod/dev
  catalogs. Missing-two-reference aggregation/candidates, explicit `--pattern-map`,
  whole-profile `--skip-unresolved`, same-ID resolution, and staged dictionary 400s
  are exercised through real CLI/SDK HTTP integration tests with **mocked catalogs**;
  they are not misreported as absent live resources. Basic profiles with null rules
  and stale pins retain fail-closed domain regression coverage.
- A separately labeled **controlled source-envelope fixture** was also checked
  against the real dev API: two predefined stub names were deliberately changed to
  nonexistent names, without changing either live catalog. Default planning exited 1
  with both misses and ranked `Passport - UK` candidates. `--skip-unresolved` skipped
  all four dependent profiles whole and reused only the existing dependencies.
  Explicitly mapping both controlled names back to `Passport - UK` verified all four
  existing profiles without writes. This exercises live API execution of the failure
  paths without pretending that prod's actual predefined names are missing.
- Profile `--pattern-id` now reads each selected pattern and builds the actual
  `sub_expressions[].rule_item` shape with the correct name, ID, technique, version,
  confidence, and threshold. The previous `condition_pattern` form was obsolete.
  Failed or retired references stop before any profile write. This was live-verified
  by source profile `11995036`, referencing newly created custom pattern
  `6aa36f8d58e04896823e0aba` and predefined `Passport - UK`.

Reproduction commands (from a directory without `.env`):

```bash
airs tenant switch prod
airs runtime dlp backup --resources dictionaries --output-file dictionaries.json
airs runtime dlp backup --resources profiles --skip-unsupported --output-file profiles.json
airs tenant switch dev
airs runtime dlp restore dictionaries.json --expect-tsg 1158365485 --force
airs runtime dlp restore profiles.json --expect-tsg 1158365485 --force
airs runtime dlp restore dictionaries.json --on-conflict verify --expect-tsg 1158365485 --force
airs runtime dlp restore profiles.json --on-conflict verify --expect-tsg 1158365485 --force
```

For an already populated destination, use `--on-conflict verify` for profiles on the
first run too. Backups remain private version-1 envelopes. Runtime AI security
profiles are not attached or modified by this DLP transfer acceptance.

### Remediation review and checks

Independent review scored the final implementation **9/10** after all identified
blocking findings were remediated. The separate pagination/transfer review scored
**9.2/10**. These are review assessments, not proof of behavior outside the recorded
API observations.

- CLI: 1,891 tests across 125 files; full coverage 97.58% statements, 89.36% branches,
  96.80% functions. Transfer module: 98.86% statements, 85.36% branches, 100% functions.
- SDK 0.30.1: 11,689 tests; 99.73% statements, 96.86% branches, 100% functions.
- Biome, TypeScript, docs production build, release-helper tests and production
  dependency audit passed. Built CLI contract tests cover named-tenant auth,
  dictionary errors, profile flags, and transfer safety, including consumer CI on
  Node 20.17.0, 22.13.0 and 24.
- A live invalid-metadata dictionary request on dev exited 1 with
  `originalFileName: must not be blank`. Its `--debug` file retained method/status
  diagnostics while omitting both bodies and all synthetic keyword/metadata content.


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
