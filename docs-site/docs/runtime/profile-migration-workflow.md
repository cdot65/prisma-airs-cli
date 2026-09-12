---
title: Full tenant migration workflow
---

# Full Runtime profile migration

This page records the earlier Basic-fallback acceptance. For the newer operator-validated
three-profile workflow preserving custom DLP through explicit dependency recreation and
mapping, use [prod to dev — preserve custom DLP](prod-dev-migration.md). The two runs have
different scopes; custom-DLP preservation does not imply that Runtime backup clones DLP resources.

This workflow backs up all latest Runtime profiles and their referenced custom topics
from `aisecurity`, restores them into an **empty** `cdot65` destination, and verifies a
second run changes nothing. It is not a complete tenant export: API keys, applications,
historical revisions, traffic, and custom Enterprise DLP resources are outside this backup.

The source is TSG `1852583913`; the destination is TSG `1220195158`. Replace both tenant
names and IDs when adapting this example. Do not use it against a production destination
without reviewing the policies and the explicit loss of custom DLP protection.

## Prerequisites

Use CLI **5.7.1 or newer** and existing named tenants with valid Management credentials.
Run commands individually from a private working directory; environment variables and
`.env` files are not read. These commands use the installed `airs` executable.

```bash
airs --version
airs runtime profiles restore --help
airs tenant list
```

Check both registered TSG IDs before continuing. The destination must be empty for this
clean-start example. These are real writes; no existing configuration is deleted.

## Inspect the destination and back up the source

```bash
airs tenant switch cdot65
airs tenant list
airs runtime profiles list --all --max 0 --output json
airs runtime topics list --all --max 0 --output json
```

Both inventories must be `[]`. Otherwise stop; use the recovery preview below for an
already-started migration rather than repeating the clean-start workflow.

```bash
airs tenant switch aisecurity
airs runtime profiles backup --all --output-file ./source-profiles.json --output json
airs tenant switch cdot65
airs tenant list
```

Verify the backup response's source TSG and counts. Keep the backup private.
Existing backup files are not overwritten; choose a new filename if necessary.

## Preview and restore

```bash
airs runtime profiles restore ./source-profiles.json \
  --on-missing-dlp basic --expect-tsg 1220195158 --dry-run --output json
```

Review every Basic fallback: **custom detection rules are lost**. Do not continue if
you require those rules; use the [custom-DLP preservation guide](prod-dev-migration.md).
Check the destination TSG, resource counts, and all proposed actions before proceeding.

```bash
airs runtime profiles restore ./source-profiles.json \
  --on-missing-dlp basic --expect-tsg 1220195158 --output json
```

Review the interactive confirmation. The result must have `complete: true`; any failure
leaves completed writes in place. Inspect rather than automatically retrying.

## Verify the restored configuration

```bash
airs runtime profiles list --all --max 0 --output json
airs runtime topics list --all --max 0 --output json
airs runtime profiles restore ./source-profiles.json \
  --on-conflict verify --on-missing-dlp basic --expect-tsg 1220195158 --dry-run --output json
```

Expect every profile action to be `verify` and topic action to be `reuse`, with no
creates or updates. Inspect individual policies with `airs runtime profiles get`:

```bash
airs runtime profiles get insomnia --output json
airs runtime profiles get NexusOS-Platform-HIGH --output json
airs runtime profiles get Truffles --output json
```

After a clean preview, complete verification and optionally back up the destination:

```bash
airs runtime profiles restore ./source-profiles.json \
  --on-conflict verify --on-missing-dlp basic --expect-tsg 1220195158 --output json
airs runtime profiles backup --all --output-file ./destination-profiles.json --output json
airs runtime profiles list --all --max 0 --output json
airs runtime topics list --all --max 0 --output json
airs tenant list
```

Expect `complete: true`, all profiles `verified`, topics `reused`, and unchanged
destination IDs/revisions. Without `--dry-run`, verify can create missing resources;
do not treat it as universally read-only. Keep source and destination backups private.

## Recovery and protection differences

If interrupted, retain the source backup and inspect a read-only recovery preview:

```bash
airs runtime profiles restore ./source-profiles.json \
  --on-conflict verify --on-missing-dlp basic --expect-tsg 1220195158 --dry-run
```

Ensure cdot65 is selected with the intended credentials. Remove `--dry-run` only after
reviewing the plan. Avoid `skip` for acceptance: it does not prove existing profiles match.

Basic fallback explicitly loses custom detection rules. In this environment it affects
`insomnia` and `Truffles` (Calvin Test), plus `NexusOS-Platform-HIGH` (Nested Tested).
Actions and masking remain verified. This test does not claim custom detection parity,
application cutover, scanner behavior or full Enterprise DLP dependency migration.

Observed server additions are checked separately: omitted detector severities, toxicity
confidence settings, topic-guardrails and blocked-topic severities. Explicit source values,
changed actions/identities/revisions, and unknown additions remain verification failures.

## Acceptance evidence

The first full attempt exposed previously untested topic severity defaults; a recovery
attempt exposed contextual-grounding severity. Both stopped rather than bypassing policy
verification. Regression tests were added for those observations. The interrupted run
then completed and verified all 19 profiles and 26 topics, with unchanged source resources.
Private recovery evidence: `artifacts/full-runtime-migration-MGdhDm/`.

### Final clean-start result — 2026-09-09

**Passed all eight acceptance checks.** After removing only this test's newly created
resources, the final run started at zero profiles and zero topics and completed without
interruption. Both source and destination credential files remained byte-for-byte unchanged;
source inventories and source backup were unchanged. `cdot65` was left selected. Restored
resources are retained for review, so the destination is **no longer empty**.

Private evidence: `artifacts/full-runtime-migration-Aodjkc/`.
The failed attempts and recovery evidence above are retained, not overwritten.

Actual aggregate output from the final run (fallback details summarized separately):

```json
{"check":"Destination identity and starting inventories checked","passed":true,"profiles":0,"topics":0,"resume":false}
{"check":"Fresh source backup created with private permissions","passed":true,"profiles":19,"topics":26}
{"check":"Dry-run creates nothing; complete dependency plan","passed":true,"profiles":19,"topics":26,"basicFallbacks":3}
{"check":"Verify rerun and final preview passed; destination IDs, revisions and policies unchanged","passed":true,"verified":19}
{"check":"Source profiles/topics and original backup unchanged; destination backup exported","passed":true}
{"check":"Credential files and tenant registrations unchanged; destination selected","passed":true}
```

The restore receipt reports **19 created, zero previously verified, 26 topics created**.
Independent comparison verified every explicit source policy value after destination topic
mapping and the three approved Basic substitutions. Unknown server additions still go
through the CLI's restrictive comparator; this independent check does not replace it.

| Profile | Custom DLP replaced | Destination protection |
| --- | --- | --- |
| insomnia | Calvin Test | Basic |
| NexusOS-Platform-HIGH | Nested Tested | Basic |
| Truffles | Calvin Test | Basic |

Use the final `--on-conflict verify --dry-run` command for a non-mutating check now.
Do not repeat the clean-start workflow unless the destination has been deliberately cleared.
This acceptance covers configuration storage and read-back, not scanning efficacy or
downstream applications switching to the new profile identities.

Regression validation: **1,680 CLI tests**, **11,667 SDK tests**, TypeScript checks and
14 release-contract tests passed. Lint passed with seven pre-existing CLI test warnings.
The updated SDK independently parsed all 19 destination policies and 26 topic references
without data changes. Those historical checks used internal acceptance tooling; the
workflow above now presents direct CLI commands without exposing that tooling.

The SDK and CLI were also packed and installed together in an isolated consumer. The same
runner with `--resume` passed all eight checks against the restored tenant: **zero profiles
created, 19 verified, zero topics created**. Independent policy checks and unchanged-source
checks passed again. Private packaged-consumer evidence:
`artifacts/full-runtime-migration-pebMFk/`. These were local package candidates, not npm releases.

### Published-package verification — 2026-09-09

SDK **0.30.0** and CLI **5.7.0** were published to npm through their GitHub release
workflows with signed provenance. A fresh registry installation resolved SDK 0.30.0;
all seven CLI package files matched the tested release candidate byte-for-byte.
The installed CLI's OAuth integration tests passed (3/3), as did all 11 native DLP
consumer checks. Those native checks used the documented local font configuration;
GitHub separately passed consumer tests on Node 20.17.0, 22.13.0 and 24.

The registry-installed CLI ran the full live acceptance runner with `--resume` against
the already-restored tenant. **All eight checks passed**, without creating resources.
Private evidence: `artifacts/full-runtime-migration-KilnEM/`.

Aggregate summary of package verification and restore receipts:

```json
{
  "cliVersion": "5.7.0",
  "sdkVersion": "0.30.0",
  "profilesCreated": 0,
  "profilesVerified": 19,
  "topicsCreated": 0,
  "checks": 8,
  "passed": true
}
```

Independent comparison again verified source preservation, destination topic bindings,
explicit policy settings, and unchanged IDs/revisions. Both credential files remained
unchanged, and `cdot65` remained selected. The global executable on the acceptance host
was updated from 5.6.0 to 5.7.0; users do not need a local repository checkout.

Release evidence: [SDK publication](https://github.com/cdot65/prisma-airs-sdk/actions/runs/34301010969),
[CLI publication](https://github.com/cdot65/prisma-airs-cli/actions/runs/34301575417),
and [CLI release checks](https://github.com/cdot65/prisma-airs-cli/actions/runs/34301476326).
The published container also passed native DLP consumer checks on both linux/amd64 and
linux/arm64, with network access disabled during verification
([container workflow](https://github.com/cdot65/prisma-airs-cli/actions/runs/34301575067)).

Assessment: **9/10 for this Runtime migration MVP**, not for complete AIRS product
coverage. Configuration migration, recovery, packaging and preservation checks pass.
The remaining gap is intentional: custom Enterprise DLP profiles, patterns and dictionaries
are not cloned, and Basic fallback cannot preserve their detection semantics. Full DLP
dependency migration and post-migration scanner-efficacy tests remain separate work.
