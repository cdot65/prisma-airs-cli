---
title: Prod to dev — preserve custom DLP
---

# Prod to dev — preserve custom DLP

Create a small Runtime configuration in `prod`, back it up, and restore it into `dev`
using only `airs` commands. No capture helpers, shell variables, or scripts are needed.
The example preserves custom DLP; see the separate
[Basic-fallback migration](profile-migration-workflow.md) if you intentionally want
to replace unavailable custom DLP with Basic protection.

## Before you start

Use CLI **5.7.1 or newer** (SDK 0.30.0 or newer), which supports named-tenant DLP
credentials. Both AIRS tenants must already exist and have Management OAuth credentials
and Enterprise DLP provisioning/permissions. `tenant create` registers a tenant locally;
it does not provision a cloud tenant.

Use dedicated test tenants with **empty Runtime profile and topic inventories** for
this example. The label `prod` is an example name, not an instruction to modify a
business production environment. These commands create real resources. Do not repeat
the setup against a tenant where this migration has already completed.

Run commands one at a time from a private working directory. Backup files contain
configuration, are created privately, and are not overwritten. No Bash-specific setup
is required. Avoid `--debug` when handling credentials or private configuration.

```bash
airs --version
airs runtime profiles restore --help
```

The restore help must include `--on-conflict` with `verify` and `--on-missing-dlp`.
Credentials come only from the selected tenant file; `PANW_*` variables,
`PRISMA_AIRS_CONFIG_PATH`, and `.env` files are ignored (`airs doctor` lists any still set).
Do not export secrets to work around an authentication failure. See [tenant authentication recovery](dlp/tenant-auth-recovery.md).

Throughout this guide, replace quoted placeholders such as `"<DEV_TSG>"` with the
actual value printed by an earlier `airs` command. They are **not environment variables**.

## 1. Register and inspect dev and prod

Enter each tenant's TSG ID, OAuth client ID, and hidden client secret when prompted.
If already registered, skip creation and inspect the existing entries instead.

```bash
airs tenant create dev
airs tenant create prod
airs tenant list
airs tenant read dev
airs tenant read prod
```

Check that `dev` and `prod` have different TSG IDs and the intended credentials/config
paths. Record the dev TSG ID for `--expect-tsg` later. Credential reads are redacted.

Check both inventories before creating anything:

```bash
airs tenant switch dev
airs tenant list
airs runtime profiles list --all --max 0 --output json
airs runtime topics list --all --max 0 --output json
airs tenant switch prod
airs tenant list
airs runtime profiles list --all --max 0 --output json
airs runtime topics list --all --max 0 --output json
```

Each profile/topic inventory should be `[]`. Stop if it is not empty; this guide does
not delete existing configuration. Successful lists also exercise authentication in
each tenant. Leave `prod` selected for the next three sections.

## 2. Create the custom DLP dependency in prod

Create a synthetic regex pattern. `AIRS-E2E-123456` is an example matching string.

```bash
airs runtime dlp patterns create \
  --name dlp-test-pattern --type custom --technique regex \
  --description 'Synthetic Runtime migration acceptance pattern' \
  --confidence-levels high --regex 'AIRS-E2E-[0-9]{6}' --output json
```

Copy the returned pattern `id` into the next command:

```bash
airs runtime dlp patterns get "<PROD_PATTERN_ID>" --output json
```

Verify the name, regex, and detection technique. Substitute that same pattern ID in the
inline request below. This is JSON passed directly to `airs`, not a shell script or a
request file. The explicit `rule_item` tree avoids the legacy `--pattern-id` shorthand,
which emits a different leaf shape.

```bash
airs runtime dlp profiles create --body '{
  "name": "dlp-test",
  "profile_type": "advanced",
  "description": "Synthetic Runtime migration acceptance DLP profile",
  "detection_rules": [{
    "rule_type": "expression_tree",
    "expression_tree": {
      "operator_type": "or",
      "sub_expressions": [{
        "rule_item": {
          "detection_technique": "regex",
          "id": "<PROD_PATTERN_ID>",
          "name": "dlp-test-pattern",
          "match_type": "include",
          "confidence_level": "high",
          "occurrence_operator_type": "any",
          "occurrence_count": 1
        }
      }]
    }
  }]
}' --output json
```

Copy the DLP profile `id` from the response and read it back:

```bash
airs runtime dlp profiles get "<PROD_DLP_PROFILE_ID>" --output json
```

Confirm `dlp-test` is advanced and active. Its `detectionRules` must contain a `ruleItem`
referencing the prod pattern, with regex/high/include/any matching. Note its `version`.
GET output uses camelCase; create request fields use snake_case. Do not replay the GET
response as a create body.

## 3. Create two custom topics in prod

```bash
airs runtime topics create \
  --name migration-financial-advice \
  --description 'Requests for personalized financial investment advice' \
  --examples 'Which stocks should I buy with my retirement savings?' \
             'Tell me how to invest my personal savings for maximum profit.'

airs runtime topics create \
  --name migration-legal-advice \
  --description 'Requests for personalized legal advice or legal representation' \
  --examples 'Should I sue my landlord over my rental dispute?' \
             'Tell me the legal strategy I should use in my court case.'

airs runtime topics list --all --max 0 --output json
```

Confirm both topic names, descriptions, and examples. Topic create/apply acknowledgements
were human-readable in the acceptance run even when JSON was requested; use the list
and GET commands for structured read-back.

## 4. Create three Runtime security profiles in prod

Create a profile with custom DLP, a profile with Basic DLP, and a profile with DLP
disabled. The third command explicitly configures latency so the CLI builds a policy
with DLP disabled rather than leaving the entire policy to server defaults.

```bash
airs runtime profiles create --name migration-custom-dlp \
  --dlp-action block --dlp-profiles dlp-test

airs runtime profiles create --name migration-basic-dlp \
  --dlp-action block --dlp-profiles 'sensitive content'

airs runtime profiles create --name migration-topics-only \
  --inline-timeout-action block --max-inline-latency 5

airs runtime topics apply \
  --profile migration-topics-only --name migration-financial-advice --intent block

airs runtime topics apply \
  --profile migration-topics-only --name migration-legal-advice --intent block

airs runtime profiles get migration-custom-dlp --output json
airs runtime profiles get migration-basic-dlp --output json
airs runtime profiles get migration-topics-only --output json
airs runtime profiles list --all --max 0 --output json
```

Inspect the stored policies before backing up. The name-based DLP flags select members
by name; they do not supply an explicit ID/version. Confirm the service stored the
intended custom dependency and compare any returned ID/version with the DLP GET above.
Stop if the reference is missing or points to a different dependency.

| Profile | Required stored configuration |
| --- | --- |
| `migration-custom-dlp` | DLP action `block`, member `dlp-test` |
| `migration-basic-dlp` | DLP action `block`, built-in member `sensitive content` |
| `migration-topics-only` | DLP action empty, member null/empty; both custom topics in a `block` group |

For the topic-only profile, guardrail-level `action: allow` permits unrelated topics;
the nested topic group must say `block`. Also review other service/default protections:
the profile flags do not reproduce every unrelated field from the historical fixture.

## 5. Back up prod Runtime configuration

Confirm `prod` is still selected, then write a new backup in the current directory:

```bash
airs tenant list
airs runtime profiles backup --all --output-file ./prod-runtime-backup.json --output json
```

Captured backup response, with the path normalized and tenant ID redacted:

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

Check the source TSG and counts. An existing output file causes `EEXIST`; choose a new
filename and use that filename in the restore commands. Do not discard a successful backup.

The backup contains latest Runtime profiles and referenced custom topics. It does
**not** contain restorable Enterprise DLP patterns/profiles, credentials, API keys,
applications, or telemetry. Prepare the custom DLP dependency separately in dev next.

## 6. Switch to dev and prepare its DLP dependency

```bash
airs tenant switch dev
airs tenant list
airs runtime dlp patterns create \
  --name dlp-test-pattern --type custom --technique regex \
  --description 'Synthetic Runtime migration acceptance pattern' \
  --confidence-levels high --regex 'AIRS-E2E-[0-9]{6}' --output json
airs runtime dlp patterns get "<DEV_PATTERN_ID>" --output json
```

Copy the **dev** pattern ID from this create response. Do not use the prod pattern ID.

```bash
airs runtime dlp profiles create --body '{
  "name": "dlp-test",
  "profile_type": "advanced",
  "description": "Synthetic Runtime migration acceptance DLP profile",
  "detection_rules": [{
    "rule_type": "expression_tree",
    "expression_tree": {
      "operator_type": "or",
      "sub_expressions": [{
        "rule_item": {
          "detection_technique": "regex",
          "id": "<DEV_PATTERN_ID>",
          "name": "dlp-test-pattern",
          "match_type": "include",
          "confidence_level": "high",
          "occurrence_operator_type": "any",
          "occurrence_count": 1
        }
      }]
    }
  }]
}' --output json
airs runtime dlp profiles get "<DEV_DLP_PROFILE_ID>" --output json
```

Compare the dev read-back with prod: same synthetic regex and matching rules, advanced
profile type, and a rule leaf using dev's pattern ID. DLP numeric profile IDs are
tenant-scoped and can happen to be equal; a matching number alone proves nothing.

This step creates equivalent dependencies explicitly; Runtime restore does not clone
Enterprise DLP. If `dlp-test` already exists, inspect it rather than creating a duplicate.
The CLI currently has no supported Enterprise DLP profile DELETE operation.

## 7. Preview and restore into dev

Replace `"<DEV_TSG>"` with dev's actual numeric TSG ID from `airs tenant list`.
Check that the destination still has no Runtime profiles or topics:

```bash
airs tenant list
airs runtime profiles list --all --max 0 --output json
airs runtime topics list --all --max 0 --output json
airs runtime profiles restore ./prod-runtime-backup.json \
  --dlp-map 'dlp-test=dlp-test' --on-missing-dlp error \
  --expect-tsg "<DEV_TSG>" --dry-run --output json
```

The preview must show the intended source/destination TSG IDs, three profile creates,
two topic creates, one DLP mapping, and no Basic fallbacks. Mapping chooses a destination
dependency; it does not check that the DLP rules are equivalent. That is why step 6
includes a read-back comparison.

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

Confirm the dry-run left the inventories empty, then execute and review the confirmation
prompt. `--force` is unnecessary for this interactive workflow.

```bash
airs runtime profiles list --all --max 0 --output json
airs runtime topics list --all --max 0 --output json
airs runtime profiles restore ./prod-runtime-backup.json \
  --dlp-map 'dlp-test=dlp-test' --on-missing-dlp error \
  --expect-tsg "<DEV_TSG>" --output json
```

The completed result must have `complete: true`, three `created` profiles, two `created`
topics, and `dlpFallbacks: []`. Stop on any error. Completed writes remain after a partial
failure; use the verification preview below to inspect recovery instead of blindly
recreating resources or choosing `skip`/`update`.

## 8. Validate the migrated configuration

```bash
airs runtime profiles list --all --max 0 --output json
airs runtime topics list --all --max 0 --output json
airs runtime profiles get migration-custom-dlp --output json
airs runtime profiles get migration-basic-dlp --output json
airs runtime profiles get migration-topics-only --output json
airs runtime dlp profiles get "<DEV_DLP_PROFILE_ID>" --output json
airs runtime dlp patterns get "<DEV_PATTERN_ID>" --output json
```

Check the following before declaring the migration successful:

| Check | Expected in dev |
| --- | --- |
| Inventory | Three profiles and two topics |
| Custom DLP | `dlp-test`, action `block`, binding to dev's DLP profile/version and rule tree |
| Basic DLP | Built-in `sensitive content`, action `block` |
| Topic-only profile | DLP disabled; both custom topics in the nested block group |
| Topic references | Dev topic IDs/revisions; descriptions and examples preserved |
| Protection changes | No Basic fallback; inspect any reported server-added defaults |

Ask restore to compare the existing profiles with the backup without changing resources:

```bash
airs runtime profiles restore ./prod-runtime-backup.json \
  --dlp-map 'dlp-test=dlp-test' --on-missing-dlp error --on-conflict verify \
  --expect-tsg "<DEV_TSG>" --dry-run --output json
```

All three profile actions should be `verify`, and both topic actions should be `reuse`.
There must be no creates or updates. A mismatch fails validation. If a resource is
missing, investigate rather than assuming the earlier restore completed.

Once that preview is clean, run the same verification without `--dry-run`:

```bash
airs runtime profiles restore ./prod-runtime-backup.json \
  --dlp-map 'dlp-test=dlp-test' --on-missing-dlp error --on-conflict verify \
  --expect-tsg "<DEV_TSG>" --output json
airs runtime profiles list --all --max 0 --output json
airs runtime topics list --all --max 0 --output json
```

Expect `complete: true`, three `verified` profiles and two `reused` topics. IDs/revisions
should remain unchanged. **Verify is not globally read-only**: without `--dry-run`, it
can create missing resources, which is why the preceding preview matters.

Optionally back up dev's restored state, inspect prod again, and leave dev selected:

```bash
airs runtime profiles backup --all --output-file ./dev-runtime-backup.json --output json
airs tenant switch prod
airs runtime profiles list --all --max 0 --output json
airs runtime topics list --all --max 0 --output json
airs tenant switch dev
airs tenant list
```

Compare prod's IDs, revisions, and configuration with the pre-migration state; migration
should not change the source. Keep both backups private. If your terminal closes, tenant
registrations and backup files persist: return to the backup directory, inspect
`airs tenant list`, select the intended tenant, and resume from the last verified step.

## Validated results — 2026-09-09

The operator's submitted migration evidence was independently checked offline:

| Stage | Verified result |
| --- | --- |
| Backup | 3 profiles, 2 referenced topics |
| Dry-run | 3 profile creates, 2 topic creates, 1 DLP mapping; destination unchanged |
| Restore | Complete; 3 profiles and 2 topics created; 0 Basic fallbacks |
| Verify rerun | Complete; 3 profiles verified, 2 topics reused; destination unchanged |
| Source | Profile/topic snapshots unchanged |
| Custom DLP | Equivalent synthetic regex and rule tree, rebound to dev dependencies |

The 45 captured commands used internal capture tooling and explicit JSON Runtime profile
requests. This guide presents direct CLI commands instead; the name-based profile flags
above are not a verbatim replay of those requests. Command syntax/request construction
is checked locally; the archived run is not evidence of a new live execution of this
edited guide. Always perform the read-back checks above, including other policy defaults.

The archive did not record the CLI version or scanner test traffic. These results validate
configuration migration, not detection efficacy or application cutover. Raw evidence,
tenant IDs, local paths, and audit identities are not published here.
