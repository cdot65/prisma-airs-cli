---
title: Full tenant migration workflow
---

# Full Runtime profile migration

This workflow backs up all latest Runtime profiles and their referenced custom topics
from `aisecurity`, restores them into an **empty** `cdot65` destination, and verifies a
second run changes nothing. It is not a complete tenant export: API keys, applications,
historical revisions, traffic, and custom Enterprise DLP resources are outside this backup.

The source is TSG `1852583913`; the destination is TSG `1220195158`. Replace both tenant
names and IDs when adapting this example. Do not use it against a production destination
without reviewing the policies and the explicit loss of custom DLP protection.

## Install the release

Use CLI **5.7.0** or later, which includes SDK **0.30.0** and the restore fixes below.
CLI 5.6.0 does not contain them. Both named tenants must already have valid Management
credentials. These commands use the installed `airs` executable, not a checkout path.

```bash
npm install --global @cdot65/prisma-airs-cli@5.7.0
airs --version
```

## Copy-and-paste workflow

The following explicitly runs Bash (also usable from a zsh terminal). Environment cleanup
is confined to that subprocess. It prevents `PANW_*`, an explicit config path or `.env`
from overriding the named tenant credentials. Credential files are read-only inputs.
Only the selected tenant changes, and the destination remains selected afterward.

**This performs real writes.** It refuses a nonempty destination and never deletes anything.
`--force` here skips the restore prompt only; `--expect-tsg` pins the destination and
name conflicts still fail. A failure stops the script; completed writes are not rolled back.

```bash
bash <<'BASH'
set -euo pipefail
command -v airs >/dev/null
for key in ${!PANW_@}; do unset "$key"; done
unset PRISMA_AIRS_CONFIG_PATH
export DOTENV_CONFIG_PATH=/dev/null
airs_local() { command airs "$@"; }
json_check() {
  node -e '
    const [mode, file] = process.argv.slice(1);
    const data = JSON.parse(require("node:fs").readFileSync(file, "utf8"));
    const checks = {
      tenants: () => [["aisecurity", "1852583913"], ["cdot65", "1220195158"]]
        .every(([name, id]) => data.some(t => t.name === name && t.tsgId === id)),
      empty: () => Array.isArray(data) && data.length === 0,
      complete: () => data[0]?.complete === true,
      verified: () => data[0]?.complete === true && data[0].profiles.length > 0
        && data[0].profiles.every(p => p.action === "verified"),
    };
    if (!checks[mode]?.()) {
      console.error(`Validation failed: ${mode} (${file})`);
      process.exit(1);
    }
  ' "$@"
}

help=$(airs_local runtime profiles restore --help)
[[ "$help" == *"--on-missing-dlp"* && "$help" == *"verify (resume"* ]]
umask 077
migration_dir=$(mktemp -d "$PWD/airs-migration-XXXXXX")
cd "$migration_dir"
echo "Private migration evidence: $migration_dir"

airs_local tenant list --output json > tenants.json
json_check tenants tenants.json

airs_local tenant switch cdot65
airs_local runtime profiles list --all --max 0 --output json > before-profiles.json
airs_local runtime topics list --all --max 0 --output json > before-topics.json
json_check empty before-profiles.json
json_check empty before-topics.json

# Always return selection to the destination, including on backup failure.
trap 'airs_local tenant switch cdot65 >&2' EXIT
airs_local tenant switch aisecurity
airs_local runtime profiles backup --all --output-file ./source-profiles.json --output json
airs_local tenant switch cdot65

airs_local runtime profiles restore ./source-profiles.json \
  --on-missing-dlp basic --expect-tsg 1220195158 --dry-run --output json > preview.json

airs_local runtime profiles restore ./source-profiles.json \
  --on-missing-dlp basic --expect-tsg 1220195158 --force --output json > restore.json
json_check complete restore.json

# Execute verification without creating new revisions of matching profiles.
airs_local runtime profiles restore ./source-profiles.json \
  --on-conflict verify --on-missing-dlp basic --expect-tsg 1220195158 \
  --force --output json > verification.json
json_check verified verification.json

airs_local runtime profiles backup --all --output-file ./destination-profiles.json --output json
airs_local runtime profiles list --all --max 0 --output json > after-profiles.json
airs_local runtime topics list --all --max 0 --output json > after-topics.json
airs_local tenant list
node -e '
  const r = JSON.parse(require("node:fs").readFileSync("restore.json", "utf8"))[0];
  console.log(JSON.stringify({complete: r.complete, profiles: r.profiles.length,
    topics: r.topics.length, basicFallbacks: r.dlpFallbacks.length}, null, 2));
'
BASH
```

The terminal warnings go to stderr and do not corrupt the JSON evidence. Private backups
and transcripts contain sensitive policy configuration and examples; do not commit them
or publish them as documentation attachments. Existing backup files are not overwritten.

## Automated acceptance runner

The repository runner performs the same CLI lifecycle plus independent checks of source
preservation, every explicit policy value (after reviewed dependency mapping and Basic
fallback), topic definitions/bindings, destination IDs/revisions and private file permissions.
It leaves restored resources in place; it never deletes or automatically retries writes.

```bash
node scripts/e2e-full-runtime-migration.mjs \
  --execute --source aisecurity --destination cdot65 \
  --expect-source-tsg 1852583913 --expect-tsg 1220195158
```

Use only when destination profiles and topics are empty. The runner saves private
`command-N.json`, source/destination backups and `results.json` under
`artifacts/full-runtime-migration-*`. An optional `AIRS_CLI_ENTRY` tests an independently
installed package. `--resume` explicitly permits matching existing resources using
`--on-conflict verify`; it is not a clean-start test and does not bypass mismatches.

## Recovery and protection differences

If interrupted, retain the source backup and inspect a read-only recovery preview:

```bash
airs runtime profiles restore ./source-profiles.json \
  --on-conflict verify --on-missing-dlp basic --expect-tsg 1220195158 --dry-run
```

Use the same clean named-tenant environment as above. Remove `--dry-run` only after
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
Do not repeat the clean-start script unless the destination has been deliberately cleared.
This acceptance covers configuration storage and read-back, not scanning efficacy or
downstream applications switching to the new profile identities.

Regression validation: **1,680 CLI tests**, **11,667 SDK tests**, TypeScript checks and
14 release-contract tests passed. Lint passed with seven pre-existing CLI test warnings.
The updated SDK independently parsed all 19 destination policies and 26 topic references
without data changes. All four Bash examples passed syntax checking; the executable
acceptance runner above supplies the live workflow evidence.

Ten documentation regression tests exercise the exact Node-based JSON checks. The exact
Bash workflow was also run against the now-populated destination and correctly stopped
at the empty-inventory guard before backup/restore. No `jq` installation is required.

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
