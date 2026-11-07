---
title: Directional profiles
sidebar_position: 4
---

# Directional security profiles

:::note Supported, for single-entry profiles
Directional profiles follow the TypeScript CLI 7.3.0 and `scm-sdk` 0.2.4. A live run against
a tenant (2026-10-22) accepted flag and `--config` creates, a protection edit of each
direction, shared-settings edits, a topic applied and reverted in each direction, and backup
and restore. Read the [known limitations](#known-limitations) before relying on them.
:::

A security profile's policy holds AI entries (`ai-security-profiles`). Two layouts exist:

- A **legacy** entry keeps its protections (`model-protection`, `app-protection`,
  `agent-protection`, `data-protection`) and its shared settings in `model-configuration`.
  Existing legacy profiles need no migration.
- A **directional** entry has `content-type-mode: per_content_type` (or a
  `content-type-configurations` key). Its protections live per direction in
  `content-type-configurations`, under `prompt`, `response`, `tool-call` and
  `tool-response`; the service requires all four keys, though a direction may be empty. Its
  `model-configuration` keeps only the
  shared settings: `latency`, `mask-data-in-storage` and
  `enable-full-conversation-inspection`.

## Creating

```bash
# A legacy profile, as before
airs runtime profiles create --name Legacy --prompt-injection block

# A directional profile with response protections
airs runtime profiles create --name Directional --direction response \
  --toxic-content 'high:block, moderate:allow' --no-enable-full-conversation-inspection

# A complete JSON body: the file names the profile; --name and every other flag are refused
airs runtime profiles create --config ./directional.json --output json
```

Directional flag creation needs at least one protection flag. Shared settings alone
(`--max-inline-latency`, `--inline-timeout-action`, `--[no-]mask-data-in-storage`,
`--[no-]enable-full-conversation-inspection`) are refused before anything is sent (exit 2),
because the service refuses a profile with no protection in any direction (the TypeScript CLI
sends that create, and the service refuses it). The flags fill the `--direction` you name;
the other three directions are written empty, so a later `update --direction` can fill any of
them. A DLP block needs `--dlp-action` on a create.

## Updating

```bash
# Edit only the response protections; every other direction and severity stays
airs runtime profiles update Directional --direction response \
  --toxic-content 'high:block, moderate:block' --no-mask-data-inline

# Shared settings need no direction and keep every direction
airs runtime profiles update Directional --max-inline-latency 8 \
  --no-mask-data-in-storage --no-enable-full-conversation-inspection

# A complete replacement (also how a direction is added, or a layout converted)
airs runtime profiles update Directional --config ./directional.json
```

| Rule | What happens |
|---|---|
| A protection edit on a directional profile | needs `--direction`, even with one direction |
| The selected direction is absent | refused; add it with a complete `--config` |
| `--direction` on a legacy profile | refused; convert it with a complete `--config` |
| Several AI entries | `--ai-profile-index <n>` (zero-based) selects one, also for shared settings; the others stay |
| An unknown `content-type-mode` | kept by shared edits; protection edits are refused |
| Detectors | merged by name: a match keeps its severities and topic lists; a name listed twice is refused |
| `--no-…` flags | write an explicit `false`; of a pair, the last one given wins |
| A setting you do not pass | stays as stored, or absent; no default is written |
| An unknown direction key | kept with its value by reads, updates, backup and restore; no direction is added |
| `--dlp-profiles`, `--[no-]mask-data-inline` | change only their fields on update; no `--dlp-action` needed |

These refusals happen after the profile is read; nothing is written and the exit code is 2.
`--direction` and `--ai-profile-index` values are checked before anything is read
(`--ai-profile-index` takes decimal digits only).

## Topics

```bash
airs runtime topics apply --profile Directional --name Restricted --intent block --direction prompt
airs runtime topics revert --profile Directional --name Restricted --direction prompt --force
```

`apply` adds the topic to the selected direction and keeps every other topic and direction.
`revert` removes it from the selected direction and deletes the topic with a plain delete. It
refuses, with nothing written or deleted, when:

- the selected direction does not reference the topic;
- another direction, another AI entry or a retained legacy block of the profile still
  references it (detach those first).

A topic still referenced by another profile (or an older revision) is kept with a warning, as
for legacy profiles.

## Reading

- `airs runtime profiles get` shows `Protection locations` (`AI[0] default / prompt, AI[0]
  default / response, …`), and `DLP tenant` (JSON `dlpTenantId`, a string) only when the
  record carries one; in the live run no `get` or `list` returned it. A value the service sends that the schema does
  not expect (a `null`, a number) is printed as sent; the TypeScript CLI refused such a read.
- A `--config` file is checked before the tenant is read: `dlp_tenant_id` must be a string,
  `content-type-mode` a string, each direction an object and
  `enable-full-conversation-inspection` a boolean, never `null` (exit 2).
- `airs runtime report` compares current protections by detector and direction, retaining their AI-entry locations and active/unknown state.
- `airs runtime profiles backup` and `restore` carry every direction: DLP mappings, the Basic
  fallback and topic remapping work per direction, and `--on-conflict verify` compares
  directions strictly. A backup keeps each record as the service sent it; a restore does not
  send the source tenant's `dlp_tenant_id`.

## Known limitations

Directional profiles are **supported, for single-entry profiles**: a live run (2026-10-22)
accepted every direction for creates, edits, topics, backup and restore.

- **Not verified live:** profiles with several AI entries (`--ai-profile-index`), DLP blocks
  inside a direction (`--dlp-action`, `--dlp-profiles`), and the Signal Brief's current protection comparison. The offline tests cover them.
- **Service rules are not checked before sending, except one.** Every directional entry needs
  all four direction keys (`prompt`, `response`, `tool-call`, `tool-response`; a flag create
  writes the ones it does not fill as `{}`, and a `--config` body must list all four); the
  `response` direction refuses `prompt-injection`; and `toxic-content` takes only `block` or
  `allow` for each level. A write that breaks one fails with HTTP 400 (exit 1), and no
  response body is shown. The exception (D336): a directional flag create with no protection
  flag (none, or shared settings only) is refused before anything is sent (exit 2), because
  the service refuses a policy with no protection in any direction. A `--config` body with
  four empty directions is still sent, and refused.
- **`dlp_tenant_id` on writes.** These writes do not send `dlp_tenant_id`: a flag
  `profiles update`, `topics apply` and `revert`, `profiles restore` (create and overwrite),
  and `profiles update --config` when the file leaves the key out. The service's answer then
  reports `dlpTenantId: "none"`; the effect on the profile's stored DLP tenant binding is
  unknown. No `get` or `list` returns the field, so the CLI cannot carry it over. This
  matches the TypeScript CLI 7.3.0 and affects legacy profiles too. A `--config` file that
  holds `dlp_tenant_id` sends it.

The behavioural contract is DESIGN §13.17 in the repository.
