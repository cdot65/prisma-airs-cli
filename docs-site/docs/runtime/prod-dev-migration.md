---
title: Prod to dev — preserve custom DLP
---

# Prod to dev — preserve custom DLP

Use CLI **5.7.1 or newer** (SDK 0.30.0 or newer). An operator completed this workflow
on 2026-09-09; the submitted archive was independently validated offline. It contains
**45 successful command receipts**, three migrated profiles and two topics, with **zero
Basic fallbacks**. The CLI version was not captured in the archive; 5.7.1 is the minimum
documented prerequisite because it fixes named-tenant DLP credentials and write output.

This page covers custom-DLP preservation, unlike the separate
[Basic-fallback migration](profile-migration-workflow.md). The archive is private;
only sanitized commands and aggregate results appear here.

For a **new acceptance run**, use each numbered section in order and inspect its output.
Do not repeat this clean-start workflow against the already-migrated dev tenant. These
commands create real resources. Use dedicated test tenants with empty Runtime profile
and topic inventories; do not point this example at a business production tenant.
No cleanup or release publication is performed by this workflow. The validation of the submitted archive made no cloud requests or changes.

Important boundaries:

- `tenant create` registers credentials for an existing AIRS tenant; it does not provision one.
- Both tenants need Management OAuth access and Enterprise DLP provisioning/permissions.
- Runtime backup includes latest security profiles and referenced custom topics, not
  credentials, API keys, applications, history, telemetry, or complete Enterprise DLP resources.
- To preserve custom DLP, this workflow explicitly creates equivalent DLP dependencies
  in dev and uses `--dlp-map`. This is not automatic DLP dependency migration.
- Enterprise DLP profiles do not currently have a supported CLI DELETE operation;
  do not assume the test DLP profile can be cleaned up like a Runtime security profile.
- Stop on any error. Inspect inventories before retrying a create: an API error does
  not prove that no resource was created. Do not use blind `skip` or `update` to pass validation.

## 0. Prepare a private workspace

Start Bash, even if your usual shell is zsh. Keep all sections in that same Bash session.
Use a fresh workspace for each independent run. No secrets belong in command arguments
or evidence files. Do not enable shell tracing (`set -x`) or `--debug` for this workflow.

```bash
bash
```

```bash
set -euo pipefail
set -o noclobber
umask 077
for key in ${!PANW_@}; do unset "$key"; done
unset PRISMA_AIRS_CONFIG_PATH
export DOTENV_CONFIG_PATH=/dev/null
workflow_dir=$(mktemp -d "$PWD/airs-prod-dev-e2e-XXXXXX")
cd "$workflow_dir"
printf 'Private evidence directory: %s\n' "$PWD"
airs --version
airs runtime profiles restore --help

# Capture arguments, stdout and stderr separately. Existing files are not overwritten.
# Do not use this wrapper for credential-entry commands.
# Saved helper definitions survive a shell exit; sourcing does not execute cloud operations.
curl --fail --location --output migration-helpers.bash \
  https://cdot65.github.io/prisma-airs-cli/examples/runtime-migration-helpers.bash
source ./migration-helpers.bash
capture cli-version.txt airs --version
```

Verify the CLI is 5.7.1 or newer and help includes both `--on-conflict verify` and
`--on-missing-dlp`. Install separately with
`npm install -g @cdot65/prisma-airs-cli@5.7.1` if necessary.

If Bash exits, your credentials and files persist. Return to this existing directory in a
new Bash session, clear the overrides as above, then run `source ./migration-helpers.bash`
and `load_tenant_ids`. Do not re-register tenants or rerun successful creates.
Retain a failed capture under a new attempt name before retrying. See
[authentication recovery](dlp/tenant-auth-recovery.md).

## 1. Onboard dev and prod through the CLI

The guided commands ask for TSG ID, client ID, and a hidden client secret one at a time.
If a name already exists, inspect it; do not replace an existing registration blindly.

```bash
airs tenant create dev
airs tenant create prod
capture tenants.json airs tenant list --output json

node -e '
  const t = JSON.parse(require("node:fs").readFileSync("tenants.json", "utf8"));
  const dev = t.find(x => x.name === "dev"), prod = t.find(x => x.name === "prod");
  if (!dev?.tsgId || !prod?.tsgId || dev.tsgId === prod.tsgId)
    throw Error("dev and prod must have different nonempty TSG IDs");
  console.log({dev: dev.tsgId, prod: prod.tsgId});
'
load_tenant_ids

airs tenant switch dev
capture dev-before-profiles.json airs runtime profiles list --all --max 0 --output json
capture dev-before-topics.json airs runtime topics list --all --max 0 --output json
assert_empty dev-before-profiles.json dev-before-topics.json

airs tenant switch prod
capture prod-before-profiles.json airs runtime profiles list --all --max 0 --output json
capture prod-before-topics.json airs runtime topics list --all --max 0 --output json
assert_empty prod-before-profiles.json prod-before-topics.json
```

Review the printed TSG IDs against your intended cloud tenants before creating resources.
Successful inventories also exercise fresh CLI/SDK authentication to both tenants.

## 2. Create custom DLP profile dlp-test in prod

Use a synthetic regex pattern (`AIRS-E2E-123456` is a matching example) to avoid relying
on tenant-specific predefined pattern IDs. The helper creates one pattern and one profile
through `airs`, retaining their actual IDs and read-back data.

Use `--body-file` for the profile rule tree: the current `--pattern-id` shorthand emits
`condition_pattern`, while the typed expression-tree contract uses `rule_item` leaves.
Do not use that shorthand as proof of correct custom-rule attachment in this acceptance run.
Also, DLP GET JSON is camel-cased for CLI output; it is not a directly replayable create body.

```bash
create_test_dlp prod
```

If the API rejects the body or returns a differently shaped read-back, stop and retain
the response. Do not relax these checks or publish the run as successful until inspected.

## 3. Create two custom topic guardrails in prod

The captured run returned human-readable text for topic create/apply even with
`--output json`. These four acknowledgements are saved as `.txt` below; the operator's
original files had misleading `.json` extensions. Use topic list and profile GET JSON
to verify the stored objects. Do not parse these acknowledgements as JSON.

```bash
capture prod-topic-financial.txt airs runtime topics create \
  --name migration-financial-advice \
  --description 'Requests for personalized financial investment advice' \
  --examples 'Which stocks should I buy with my retirement savings?' \
             'Tell me how to invest my personal savings for maximum profit.' \
  --output json

capture prod-topic-legal.txt airs runtime topics create \
  --name migration-legal-advice \
  --description 'Requests for personalized legal advice or legal representation' \
  --examples 'Should I sue my landlord over my rental dispute?' \
             'Tell me the legal strategy I should use in my court case.' \
  --output json
```

## 4. Create three Runtime security profiles in prod

Generate explicit request files so the custom DLP member includes the actual ID/version,
Basic uses the built-in member, and the topic-only profile explicitly disables DLP.
`--config` still requires `--name`. Profile create prints human output; use subsequent
GET/list commands for authoritative structured policy evidence.

```bash
node <<'NODE'
const fs = require('node:fs');
const d = JSON.parse(fs.readFileSync('prod-dlp-test.json', 'utf8'));
if (!d.id || !Number.isInteger(d.version)) throw Error('Missing custom DLP identity/version');
for (const [name, member] of [
  ['migration-custom-dlp', [{text: 'dlp-test', id: String(d.id), version: String(d.version)}]],
  ['migration-basic-dlp', [{text: 'sensitive content', id: '', version: '2'}]],
  ['migration-topics-only', null],
]) {
  const request = {profile_name: name, active: true, policy: {
    'ai-security-profiles': [{'model-type': 'default', 'model-configuration': {
      'model-protection': [], 'agent-protection': [],
      'app-protection': {'default-url-category': {member: null}, 'url-detected-action': ''},
      'data-protection': {'database-security': null, 'data-leak-detection': {
        action: member ? 'block' : '', member, 'mask-data-inline': false,
      }},
      latency: {'inline-timeout-action': 'block', 'max-inline-latency': 5},
      'mask-data-in-storage': false,
    }}],
    'dlp-data-profiles': [],
  }};
  fs.writeFileSync(`${name}.request.json`, JSON.stringify(request, null, 2), {flag: 'wx', mode: 0o600});
}
NODE

for name in migration-custom-dlp migration-basic-dlp migration-topics-only; do
  capture "prod-$name-create.txt" airs runtime profiles create \
    --name "$name" --config "$name.request.json"
done

capture prod-apply-financial.txt airs runtime topics apply \
  --profile migration-topics-only --name migration-financial-advice --intent block --output json
capture prod-apply-legal.txt airs runtime topics apply \
  --profile migration-topics-only --name migration-legal-advice --intent block --output json

capture prod-profiles.json airs runtime profiles list --all --max 0 --output json
capture prod-topics.json airs runtime topics list --all --max 0 --output json
for name in migration-custom-dlp migration-basic-dlp migration-topics-only; do
  capture "prod-$name.json" airs runtime profiles get "$name" --output json
done
```

In the topic-only policy, the guardrail-level action is `allow` (allow unrelated topics),
while both topic references are inside a `block` group. That is the intended block-list
configuration, not an accidentally allowed topic.

## 5. Back up prod Runtime configuration

```bash
capture prod-backup-summary.json airs runtime profiles backup --all \
  --output-file "$PWD/prod-runtime-backup.json" --output json

node -e '
  const fs = require("node:fs"), assert = require("node:assert/strict");
  const r = JSON.parse(fs.readFileSync("prod-backup-summary.json", "utf8"))[0];
  const tenants = JSON.parse(fs.readFileSync("tenants.json", "utf8"));
  const expected = tenants.find(t => t.name === "prod")?.tsgId;
  assert.ok(expected, "Missing prod registration receipt");
  assert.equal(r.sourceTsgId, expected);
  assert.equal(r.profiles, 3); assert.equal(r.topics, 2);
  console.log("Expected source backup: 3 profiles, 2 referenced topics");
'
```

The DLP pattern/profile requests and read-back files captured in section 2 are additional
dependency evidence. They are **not** inside `prod-runtime-backup.json` as restorable
Enterprise DLP resources.

Captured backup response, with the private path normalized and tenant ID redacted:

```json
[
  {
    "file": "./prod-runtime-backup.json",
    "sourceTsgId": "<PROD_TSG>",
    "profiles": 3,
    "topics": 2
  }
]
```

If the backup succeeds but a shell assertion reports an undefined expected TSG, preserve
the backup. The validation above reads the saved registration receipt instead of relying
on an exported shell variable; do not rerun backup or overwrite its evidence.

## 6. Switch to dev, prepare its DLP dependency, and dry-run

Re-create the same synthetic rule with dev's own pattern ID. Never copy prod's pattern
or DLP IDs into the dev create request. The helper regenerates the request from dev's
read-back. No Runtime security profiles or custom topics are created by this helper.

```bash
load_tenant_ids
airs tenant switch dev
capture dev-selected.json airs tenant list --output json
create_test_dlp dev

capture dev-pre-restore-profiles.json airs runtime profiles list --all --max 0 --output json
capture dev-pre-restore-topics.json airs runtime topics list --all --max 0 --output json
assert_empty dev-pre-restore-profiles.json dev-pre-restore-topics.json

capture dev-preview.json airs runtime profiles restore ./prod-runtime-backup.json \
  --dlp-map 'dlp-test=dlp-test' --on-missing-dlp error \
  --expect-tsg "${DEV_TSG:?Run load_tenant_ids first}" --dry-run --output json

capture dev-after-preview-profiles.json airs runtime profiles list --all --max 0 --output json
capture dev-after-preview-topics.json airs runtime topics list --all --max 0 --output json
assert_empty dev-after-preview-profiles.json dev-after-preview-topics.json

node -e '
  const r = JSON.parse(require("node:fs").readFileSync("dev-preview.json", "utf8"))[0];
  const assert = require("node:assert/strict");
  assert.equal(r.destinationTsgId, process.env.DEV_TSG);
  assert.equal(r.dryRun, true);
  assert.equal(r.profiles.length, 3); assert.ok(r.profiles.every(p => p.action === "create"));
  assert.equal(r.topics.length, 2); assert.ok(r.topics.every(t => t.action === "create"));
  assert.equal(r.dlpMappings.length, 1); assert.equal(r.dlpFallbacks.length, 0);
  console.log("Preview passed: 3 creates, 2 topic creates, 1 DLP mapping, no fallback");
'
```

Review this preview before executing the next section. Mapping by itself only selects
an existing destination DLP profile; it does not prove rule equivalence. Section 2's
read-back checks are required on both tenants.

Captured preview response (tenant IDs redacted):

```json
[
  {
    "sourceTsgId": "<PROD_TSG>",
    "destinationTsgId": "<DEV_TSG>",
    "dryRun": true,
    "profiles": [
      {"name": "migration-basic-dlp", "action": "create"},
      {"name": "migration-custom-dlp", "action": "create"},
      {"name": "migration-topics-only", "action": "create"}
    ],
    "topics": [
      {"name": "migration-financial-advice", "action": "create"},
      {"name": "migration-legal-advice", "action": "create"}
    ],
    "dlpMappings": [{"source": "dlp-test", "destination": "dlp-test"}],
    "dlpFallbacks": [],
    "serverDefaults": []
  }
]
```

## 7. Execute the restore into dev

`--force` skips the confirmation prompt so stdout can be captured reliably. It does not
override conflicts, missing dependencies, verification failures, or the destination TSG check.

```bash
load_tenant_ids
capture dev-restore.json airs runtime profiles restore ./prod-runtime-backup.json \
  --dlp-map 'dlp-test=dlp-test' --on-missing-dlp error \
  --expect-tsg "${DEV_TSG:?Run load_tenant_ids first}" --force --output json

node -e '
  const r = JSON.parse(require("node:fs").readFileSync("dev-restore.json", "utf8"))[0];
  const assert = require("node:assert/strict");
  assert.equal(r.complete, true);
  assert.equal(r.profiles.length, 3); assert.ok(r.profiles.every(p => p.action === "created"));
  assert.equal(r.topics.length, 2); assert.ok(r.topics.every(t => t.action === "created"));
  assert.equal(r.dlpFallbacks.length, 0);
  console.log("Restore completed: 3 profiles, 2 topics, no Basic substitutions");
'
```

If interrupted, do not delete the evidence or rerun creation blindly. Use a new output
filename and `--on-conflict verify --dry-run` with the same DLP mapping to inspect recovery.

## 8. Validate the migration and a write-free rerun

```bash
capture dev-profiles.json airs runtime profiles list --all --max 0 --output json
capture dev-topics.json airs runtime topics list --all --max 0 --output json
for name in migration-custom-dlp migration-basic-dlp migration-topics-only; do
  capture "dev-$name.json" airs runtime profiles get "$name" --output json
done

node <<'NODE'
const fs = require('node:fs'), assert = require('node:assert/strict');
const read = f => JSON.parse(fs.readFileSync(f, 'utf8'));
function checkTenant(prefix) {
  const profiles = read(`${prefix}-profiles.json`), topics = read(`${prefix}-topics.json`);
  assert.equal(profiles.length, 3); assert.equal(topics.length, 2);
  const config = name => {
    const p = read(`${prefix}-${name}.json`);
    assert.equal(p.active, true);
    return p.policy['ai-security-profiles'][0]['model-configuration'];
  };
  const custom = config('migration-custom-dlp')['data-protection']['data-leak-detection'];
  const dlp = read(`${prefix}-dlp-test.json`);
  assert.equal(custom.action, 'block'); assert.equal(custom.member.length, 1);
  assert.equal(custom.member[0].text, 'dlp-test');
  assert.equal(String(custom.member[0].id), String(dlp.id));
  assert.equal(String(custom.member[0].version), String(dlp.version));
  const basic = config('migration-basic-dlp')['data-protection']['data-leak-detection'];
  assert.equal(basic.action, 'block'); assert.equal(basic.member.length, 1);
  assert.equal(basic.member[0].text, 'sensitive content');
  assert.equal(basic.member[0].id ?? '', '');
  assert.equal(basic.member[0].version ?? '2', '2');
  const topicConfig = config('migration-topics-only');
  const disabled = topicConfig['data-protection']['data-leak-detection'];
  assert.equal(disabled.action, '');
  assert.equal(disabled.member?.length ?? 0, 0);
  const guard = topicConfig['model-protection'].find(x => x.name === 'topic-guardrails');
  assert.equal(guard.action, 'allow');
  const blocked = guard['topic-list'].filter(g => g.action === 'block').flatMap(g => g.topic);
  assert.deepEqual(blocked.map(t => t.topic_name).sort(),
    ['migration-financial-advice', 'migration-legal-advice']);
  for (const ref of blocked) {
    const topic = topics.find(t => t.topicName === ref.topic_name);
    assert.equal(ref.topic_id, topic.topicId);
    assert.equal(ref.revision, topic.revision);
  }
  return {profiles, topics};
}
const prod = checkTenant('prod'), dev = checkTenant('dev');
for (const p of prod.profiles) {
  const d = dev.profiles.find(x => x.profileName === p.profileName);
  assert.notEqual(d.profileId, p.profileId);
}
for (const t of prod.topics) {
  const d = dev.topics.find(x => x.topicName === t.topicName);
  assert.notEqual(d.topicId, t.topicId);
  assert.equal(d.description, t.description);
  assert.deepEqual(d.examples, t.examples);
}
console.log('PASS: custom DLP bound to dev, Basic preserved, DLP disabled on topic-only profile, both topics blocked and rebound');
NODE

capture dev-verify.json airs runtime profiles restore ./prod-runtime-backup.json \
  --dlp-map 'dlp-test=dlp-test' --on-missing-dlp error --on-conflict verify \
  --expect-tsg "${DEV_TSG:?Run load_tenant_ids first}" --force --output json
capture dev-after-verify-profiles.json airs runtime profiles list --all --max 0 --output json
capture dev-after-verify-topics.json airs runtime topics list --all --max 0 --output json

node <<'NODE'
const fs = require('node:fs'), assert = require('node:assert/strict');
const read = f => JSON.parse(fs.readFileSync(f, 'utf8'));
const r = read('dev-verify.json')[0];
assert.equal(r.complete, true); assert.equal(r.profiles.length, 3);
assert.ok(r.profiles.every(p => p.action === 'verified'));
assert.equal(r.dlpFallbacks.length, 0);
for (const kind of ['profiles', 'topics']) {
  const key = kind === 'profiles' ? 'profileId' : 'topicId';
  const sort = rows => [...rows].sort((a,b) => String(a[key]).localeCompare(String(b[key])));
  assert.deepEqual(sort(read(`dev-${kind}.json`)), sort(read(`dev-after-verify-${kind}.json`)));
}
console.log('PASS: all three profiles verified; destination IDs, revisions and configurations unchanged');
NODE

capture dev-backup-summary.json airs runtime profiles backup --all \
  --output-file "$PWD/dev-runtime-backup.json" --output json

# Verify the source was not changed by migration, then return selection to dev.
airs tenant switch prod
capture prod-after-profiles.json airs runtime profiles list --all --max 0 --output json
capture prod-after-topics.json airs runtime topics list --all --max 0 --output json
airs tenant switch dev
node <<'NODE'
const fs = require('node:fs'), assert = require('node:assert/strict');
const read = f => JSON.parse(fs.readFileSync(f, 'utf8'));
for (const kind of ['profiles', 'topics']) {
  const key = kind === 'profiles' ? 'profileId' : 'topicId';
  const sort = rows => [...rows].sort((a,b) => String(a[key]).localeCompare(String(b[key])));
  assert.deepEqual(sort(read(`prod-${kind}.json`)), sort(read(`prod-after-${kind}.json`)));
}
console.log('PASS: prod profiles and topics unchanged');
NODE
capture final-tenants.json airs tenant list --output json
```

Validated acceptance matrix (configuration read-back):

| Runtime profile | Prod | Dev |
| --- | --- | --- |
| migration-custom-dlp | Custom dlp-test, block | Dev dlp-test ID/version, block |
| migration-basic-dlp | Built-in Basic, block | Built-in Basic, block |
| migration-topics-only | DLP disabled; two blocked topics | DLP disabled; both dev topics blocked |

This verifies configuration creation, storage, migration, mapping and idempotence. It
does not prove scanner efficacy. Actual inference/scan tests would require separate
Runtime scan keys, profile selection and test traffic, which are outside this run.

## Optional: test Basic fallback instead of custom-DLP preservation

Do not mix this branch into the primary preservation run. Use an empty destination
or a separately named test run. Skip dev DLP dependency creation and omit `--dlp-map`:

```bash
airs runtime profiles restore ./prod-runtime-backup.json \
  --on-missing-dlp basic --expect-tsg "${DEV_TSG:?Run load_tenant_ids first}" --dry-run --output json
```

After reviewing the warning, execute without `--dry-run` and with `--force`. Expected:
one Basic fallback for migration-custom-dlp; the existing Basic case remains Basic and
the topic-only case remains DLP-disabled. The custom-DLP assertions in the primary
validation block intentionally do not pass in this lossy mode. Record it as a distinct
fallback acceptance run, not as preservation of the custom rule.

## Validated operator evidence — 2026-09-09

The review checked all 45 exit-code receipts and their command/output/diagnostic companions,
both Runtime backup files, DLP pattern/profile read-backs and full profile/topic inventories.
Four topic acknowledgements were text, not JSON; the required structured evidence parsed.
This is validation of operator-supplied snapshots, not a fresh replay against the live API.

Sanitized aggregate projection of the captured receipts (not verbatim CLI output):

```json
{
  "source": "prod",
  "destination": "dev",
  "capturedCommands": 45,
  "backup": {"profiles": 3, "topics": 2},
  "dryRun": {"profileCreates": 3, "topicCreates": 2, "dlpMappings": 1},
  "restore": {"complete": true, "profilesCreated": 3, "topicsCreated": 2, "basicFallbacks": 0},
  "verify": {"complete": true, "profilesVerified": 3, "topicsReused": 2},
  "destinationAfterDryRun": {"profiles": 0, "topics": 0},
  "sourceSnapshotsUnchanged": true,
  "destinationSnapshotsUnchangedAfterVerify": true,
  "finalSelectedTenant": "dev"
}
```

The independent review also verified:

- Every stored policy value matches after rebinding only the custom DLP member and topic
  references; no additional server-default differences were needed in this fixture.
- Both topic descriptions/examples match, with new dev topic IDs and correct revisions.
- All three dev profile IDs differ from prod; the verify rerun preserves dev IDs/revisions.
- Both backups match their respective profile policies and topic definitions.
- The synthetic regex, matching configuration and DLP rule tree are equivalent after
  mapping the source pattern ID to the destination pattern ID.
- The DLP profile numeric ID happened to be identical in both tenants. It is **tenant-scoped**:
  DLP tenant identities and pattern IDs differ. A matching number alone does not prove
  a valid binding, nor should acceptance require different DLP profile numbers.

The archive does not include credential files, CLI version output, scanner test traffic,
or a signed command log. It cannot prove credential-file byte preservation, scanning
efficacy, or application cutover. The shell's earlier unset-TSG assertion failures were
outside `capture`; they are described as troubleshooting history, not included in the
45 successful CLI receipts.

To independently recheck this exact three-profile fixture from the CLI repository:

```bash
node scripts/validate-prod-dev-evidence.mjs /path/to/logs.tar.gz
```

The validator reads archive members without extracting or executing them, makes no API
calls, rejects mismatches, and prints only aggregate checks plus the archive SHA-256.
It requires Node and `tar`; it is a fixture validator, not a general tenant health checker.
The source archive was not copied into the repository.

## Keeping evidence private and resumable

`capture` writes four files per command: output, `.command.txt`, `.stderr`, and
`.exit-code.txt`. A zero exit is necessary but not sufficient; keep the read-back and
before/after comparisons too. The helper refuses existing output names before invoking
a command. Retain failed attempts separately; never silently overwrite them.

Keep the raw archive, tenant receipts (local credential paths), backups, identities and
audit fields private. For Docusaurus, publish only reviewed excerpts and explicit
redactions. The guide now records `cli-version.txt` in future runs, and the saved helper
reloads exported `PROD_TSG` / `DEV_TSG` from the registered tenants, addressing the
shell-state failures encountered during this run.
