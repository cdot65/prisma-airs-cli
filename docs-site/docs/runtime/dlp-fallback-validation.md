---
title: DLP fallback MVP acceptance
---

# DLP fallback MVP acceptance — 2026-09-08

## Full migration completed — 2026-09-09

The final clean-start workflow now passes for **all 19 real source profiles and 26 topics**,
including three explicitly approved Basic DLP fallbacks. A second restore verifies all 19
without changing destination IDs, revisions or policies. The source and credential files
are unchanged; cdot65 is selected and contains the restored configuration.
See the [full workflow and actual evidence](./profile-migration-workflow.md).
Private receipt: `artifacts/full-runtime-migration-Aodjkc/`. Earlier partial-restore states
below are historical and are superseded by this successful clean-start run.

Included in CLI **5.7.0** with SDK **0.30.0**. Historical results below are dated stages
of validation; the complete migration evidence is linked above. CLI 5.6.0 does not
include these fixes.

## Latest follow-up: nested toxicity — 2026-09-09

The user's next restore reached `Claude Code` and stopped after creating eight profiles
in total. The API added confidence severities to each of its eight toxicity categories.
The SDK now models these nested fields. The CLI accepts only the observed omitted-source
pair `{high: "medium", moderate: "low"}` under a matching toxicity detector and category.
Explicit values, action changes, unknown defaults and changed category arrays still fail.

The expanded live runner passed with **five** synthetic policies: the previous three,
plus nested categories with omitted severities and nested categories with explicit overrides.
Read-back verified all policies, actions and masking. A second restore verified all five
without changing IDs, revisions or policies. All five synthetic profiles were removed;
absence, unchanged tenant selection and unchanged credential files were verified.
Private evidence: `artifacts/tenant-profiles-e2e-dlp-mnnECf/`.

Both local package tarballs were installed in an isolated consumer directory. The packed
CLI matched the built payload and resolved the SDK's new category schema. All three
CLI/OAuth integration tests and the five-policy live workflow passed again, followed by
verified cleanup of all five consumer-test profiles. Evidence:
`artifacts/tenant-profiles-e2e-dlp-cOMPfl/`. No tarball was published.

Actual aggregate live output:

```json
{"check":"Live API accepted five Basic policies including nested toxicity; read-back and masking verified","passed":true,"profiles":5,"fallbacks":1}
{"check":"Explicit severities preserved; verified rerun leaves IDs, revisions and policies unchanged","passed":true}
{"check":"Only owned synthetic profiles removed; absence verified","passed":true,"removed":5}
{"check":"Tenant configuration and selection unchanged","passed":true}
```

The **read-only real-backup preview** now verifies all eight existing profiles, plans
11 creations, reuses 26 topics and lists three Basic DLP fallbacks. `Claude Code` reports
ten accepted additions: eight category confidence maps, URL severity and prompt-injection
severity. The agent has **not executed** the remaining migration or changed existing profiles.

```text
Profiles: verify=8, create=11
Topics: reuse=26
Basic DLP fallbacks: 3
Claude Code: 10 verified server-added fields
```

Full regression: **1,662 CLI tests** passed; TypeScript, lint (seven pre-existing warnings),
14 release-contract tests and the CLI Docusaurus build passed. SDK regression: **11,666 tests**
passed. These checks do not imply that the remaining 11 real profile writes have been tested.

## Follow-up: full-policy verification and recovery

The initial minimal fixtures below did not cover all fields in the user's real backup.
The user's subsequent restore created 26 topics and one profile, then stopped because
the server added ten severity fields absent from the source. This was not a completed
migration. The SDK now has separate local typed models for those observed extensions;
the CLI comparison must still explicitly handle permitted server additions.

The updated live runner uses **three rich synthetic policies**, exercising database,
source-code, malicious-code, URL, prompt-injection, toxicity and agent protection:

- Existing Basic DLP with omitted severities: API-added values are checked and reported.
- Custom-to-Basic fallback with the same protections: masking and actions are preserved.
- Basic DLP with explicit severity overrides: all configured severity values survive.
- Repeating with `--on-conflict verify` verifies all three without changing their IDs,
  revisions or policies. All three synthetic profiles are then removed; absence verified.

One expanded test first stopped on a newly observed empty default URL category. That
single synthetic profile was removed. Verification now accepts only the exact observed
empty addition, not arbitrary URL categories. The next rich-policy run passed all checks.
Private evidence: `artifacts/tenant-profiles-e2e-dlp-iCdjco/`.

The CLI and updated SDK were also packed into local tarballs and installed together in
an isolated consumer directory. Package payload comparison, new SDK schema resolution,
all three CLI/OAuth integration tests, and the same live rich-policy/recovery workflow
passed. The three consumer-test profiles were removed, with absence verified. Evidence:
`artifacts/tenant-profiles-e2e-dlp-iaLoib/`. These tarballs are **local candidates**, not npm
releases; version metadata has not yet been bumped.

The final CLI regression run passed **1,653 tests**. Type-checking, lint (seven existing
unrelated warnings), 14 release-contract checks and the Docusaurus build passed. The new
directional policy comparator has 100% statement/function coverage and 56/57 covered
branches. The SDK's separate model validation passed 11,658 tests as recorded in its docs.

A separate **read-only** recovery preview against the real backup verifies `adamb-profile`,
reuses all 26 topics, and plans only the remaining 18 profile creations with three Basic
fallbacks. Ten server-added severity fields are reported for the existing profile. No
user destination resources were changed during this verification.

```bash
airs runtime profiles restore ./profiles.json --on-conflict verify \
  --on-missing-dlp basic --expect-tsg <destination-tsg> --dry-run --output json
```

Remove `--dry-run` only after reviewing the plan. This recovery mode is currently in the
local build, not npm CLI 5.6.0. The exact server-value checks are intentionally narrow:
unexpected additions or altered explicit settings remain verification failures.

## Actual live CLI output

The live runner used an in-memory-generated backup with a source TSG different from the
authenticated cloud destination. It restored two uniquely named synthetic profiles:
one already Basic and one referencing a deliberately missing custom DLP profile.
Both used block action and inline masking. These are configuration acceptance tests,
not evidence that Basic reproduces the source custom detector's behavior.

Command: `node scripts/e2e-dlp-fallback.mjs` after `pnpm build`.
Named tenants default to `aisecurity` and `cdot65`; override with
`AIRS_DLP_SOURCE_TENANT` and `AIRS_DLP_DEST_TENANT`.

Actual successful runner output (artifact path omitted):

```json
[
  {
    "check": "Live destination preflight preserves Basic and plans one custom fallback",
    "passed": true
  },
  {
    "check": "Live API accepted both Basic policies; read-back and masking verified",
    "passed": true,
    "profiles": 2,
    "fallbacks": 1
  },
  {
    "check": "Only owned synthetic profiles removed; absence verified",
    "passed": true,
    "removed": 2
  },
  {
    "check": "Tenant configuration and selection unchanged",
    "passed": true
  }
]
```

Two earlier attempts exposed an API normalization: omitted `database-security` comes
back as `null`. Both runs failed strict read-back verification and cleaned up their one
created synthetic profile. The verifier now normalizes only that known optional field;
the next run passed. All synthetic resources were removed and their absence verified.

## Existing backup preview

A separate read-only run against the user's 19-profile, 26-topic backup asserted the
destination TSG and used a collision-avoidance name prefix. It planned all 19 profiles
and 26 topics successfully, with exactly three Basic fallbacks. Existing Basic DLP
did not require a custom dependency mapping. No profiles or topics from this backup
were created or updated. The source backup and tenant selection remain unchanged.

## Assessment and limitations

- Full unit/integration coverage run passed: 1,638 tests; configured coverage gates passed.
- TypeScript checking, bundled CLI build, 14 release-contract tests and Docusaurus build
  passed. Lint passed with seven existing non-null-assertion warnings in an unrelated test.
- Isolated two-tenant CLI/OAuth tests cover default refusal, all six output formats,
  warning separation, dry-run without writes, confirmed restore and input preservation.
  The final bundled-CLI integration and focused migration regression run passed 52 tests.
- Tests cover mapping priority, missing and ambiguous targets, API errors, skipped profiles,
  disabled DLP, masking constraints and complete reporting of discarded custom dependencies.
- Test registries now use private temporary directories; the user's active tenant cannot
  change unit/integration test behavior or be modified by those tests.
- Live acceptance verifies policy storage and read-back, not detection parity or propagation.
- Recursive migration of DLP profiles, nested profiles, patterns, dictionaries and keyword
  files remains deferred. No shared DLP resources were created, changed or deleted.
- The fallback is explicit because Basic does not preserve custom rules. Keep the original
  backup and use reviewed destination mappings to restore Advanced protection later.

MVP assessment: ready for local review, with the above limits. Full dependency migration
is not complete and is not represented as such. See the [migration guide](./profile-transfer.md)
for the flag and safety behavior.
