---
sidebar_label: profiles
---

# runtime profiles

## Legacy and directional policies

Legacy profiles keep protections and shared settings under
`policy.ai-security-profiles[].model-configuration`. Directional profiles keep shared
latency, storage masking, and conversation inspection there, and put protections in
`content-type-configurations` with `content-type-mode: per_content_type`. Existing
legacy profiles need no migration. Directions are `prompt`, `response`, `tool-call`,
and `tool-response`; the service may omit directions or individual detectors.

```bash
# Existing legacy flag behavior
airs-cli runtime profiles create --name Legacy --prompt-injection block

# A new profile with response-specific protections
airs-cli runtime profiles create --name Directional --direction response \
  --toxic-content 'high:block, moderate:allow' \
  --no-enable-full-conversation-inspection --output json

# Edit only response protections; preserve all other directions and detector severities
airs-cli runtime profiles update Directional --direction response \
  --toxic-content 'high:alert, moderate:allow' --no-mask-data-inline

# Shared settings need no direction
airs-cli runtime profiles update Directional --max-inline-latency 8 \
  --no-mask-data-in-storage --no-enable-full-conversation-inspection

# Complete JSON create or deliberate layout replacement
airs-cli runtime profiles create --config ./directional.json --output json
airs-cli runtime profiles update Legacy --config ./directional.json --output json
```

`--config` uses the SDK request shape (`profile_name`, `active`, `policy`), validates
before OAuth, and supplies a complete replacement on update. It supports both layouts
and is exclusive with identity, protection, shared-setting, and selector flags.
`--name` is required for flag-based creation; JSON creation reads `profile_name` from
its file. JSON/YAML output retains the full policy and distinguishes directions.
Response metadata `dlp_tenant_id` is exposed as optional `dlpTenantId`; GET may omit it.

Protection edits on existing directional profiles require `--direction`, even when
only one direction is present. Protection and topic edits require that selected
direction to exist; add a missing direction with a complete `--config` replacement.
Shared-only updates preserve omitted directions. Selecting a direction on a legacy
update refuses;
use complete JSON to convert a layout. When a policy contains multiple AI entries,
select an existing entry with `--ai-profile-index <n>` (zero-based), including for
shared settings. The selected entry's model identity and all other entries survive.
Unknown future mode strings are retained in JSON and transfer; protection flag edits
refuse modes whose behavior has not been established.

Positive and negative boolean flags preserve explicit true/false values.
`--mask-data-inline` / `--no-mask-data-inline` can change only masking on update,
preserving the existing DLP action and members. Directional flag creation requires
at least one protection or shared setting; use
JSON for deliberately empty configurations. Creation must also supply a DLP action
when creating a DLP detection block. Flag updates merge detectors by name and overlay
nested settings, preserving unmentioned severities, topic references, and future fields.

Directional topics use the same selectors:

```bash
airs-cli runtime topics apply --profile Directional --name Restricted \
  --intent block --direction prompt
airs-cli runtime topics revert --profile Directional --name Restricted \
  --direction prompt --force
```

Apply preserves other topics and directions. Revert refuses when the topic is still
referenced by another direction or AI entry in the profile; deletion in both layouts
uses the service's normal reference checks instead of force-removing references in
other profiles. Read-only topic lookup without a selector includes all active
protection locations. Library callers can request `{ includeInactive: true }` as its
third argument to inspect retained legacy references. The exported
`profileProtectionLocations` helper includes all locations with identity, direction,
and an active marker. Daily reports label each AI entry and direction separately
from shared latency/storage settings.

### SDK availability

CLI 7.3.0 pins registry-published SDK **0.35.0**, which contains the directional
schemas. The release checks validate the installed SDK contract and a clean package
installation; local development packages are not used in the release. Published SDK
0.34.0 does not contain these schemas. CLI validation uses local transport tests and
does not claim live service acceptance of profile mutations.

### runtime profiles backup and restore

```bash
airs-cli runtime profiles backup --all --output-file ./profiles.json
airs-cli runtime profiles backup "Production" --file-format yaml --output-file ./production.yaml
airs-cli runtime profiles restore ./profiles.json --dry-run --output json
airs-cli runtime profiles restore ./profiles.json --expect-tsg 200 --force
```

Backup exports latest policies and exact referenced topic definitions into a private,
no-overwrite file in the current directory. Restore targets the selected tenant and
rewrites topic identities. Use `airs-cli tenant switch <name>` to change tenants first.
Replace `200` with your destination TSG.

| Flag | Command | Meaning |
| --- | --- | --- |
| `[profile]` / `--all` | backup | Exact name/ID, or all latest profiles (default) |
| `--file-format json\|yaml` | backup | File encoding; default JSON |
| `--output-file <path>` | backup | New file; unique CWD filename by default |
| `--dry-run` | restore | Validate and read destination without mutations |
| `--name-prefix <prefix>` | restore | Prefix both profile and topic names |
| `--on-conflict error\|verify\|skip\|update` | restore | Default error; verify mode (5.7.0+) checks existing profiles without updating them, then creates missing profiles |
| `--dlp-map <source=destination>` | restore | Repeatable cross-tenant DLP binding to an existing target profile |
| `--on-missing-dlp error\|basic` | restore | MVP (5.7.0+): default error; explicitly accept Basic detection instead of unresolved custom DLP |
| `--expect-tsg <id>` | restore | Assert destination; mandatory with `--force` |
| `--force` | restore | Skip confirmation, not conflict/validation checks |
| `--max-pages <n>` | both | 1–1000; default 100; incomplete inventories fail |
| `--output <format>` | both | Summary: pretty, table, markdown, csv, json, yaml |

See [profile migration and live E2E evidence](../../runtime/profile-transfer.md) for
the full workflow, DLP limitations, partial-failure handling, and actual backup output.

---

### runtime profiles list

List security profiles

```text
airs-cli runtime profiles list [options]
```

#### Options

| Flag | Required | Default | Description |
|------|:--------:|---------|-------------|
| `--limit <n>` | No | `100` | Max results |
| `--offset <n>` | No | `0` | Starting offset |
| `--all` | No | — | Walk every page |
| `--max <n>` | No | `10000` | Safety cap for `--all`; `0` removes the cap |
| `--all-versions` | No | — | Include historical revisions instead of latest-only results |
| `--output <format>` | No | Resolved | Output format: pretty, table, markdown, csv, json, yaml |

#### Examples

*Pretty output (fallback `pretty`)*

```bash
airs-cli runtime profiles list --limit 2
```

```text
Prisma AIRS — Runtime Configuration
Security profile and topic management


Security Profiles:

00000000-0000-0000-0000-000000000001
  docs-example-profile  active rev:1
00000000-0000-0000-0000-000000000002
  example-other-profile  active rev:6

Next offset: 2
```

*JSON output*

```bash
airs-cli runtime profiles list --limit 2 --output json
```

```text
[
  {
    "profileId": "00000000-0000-0000-0000-000000000001",
    "profileName": "docs-example-profile",
    "active": true,
    "revision": 1
  },
  {
    "profileId": "00000000-0000-0000-0000-000000000002",
    "profileName": "example-other-profile",
    "active": true,
    "revision": 6
  }
]
```

*YAML output (one sequence containing complete records)*

```bash
airs-cli runtime profiles list --limit 2 --output yaml
```

```text
- profileId: 00000000-0000-0000-0000-000000000001
  profileName: docs-example-profile
  active: true
  revision: 1
- profileId: 00000000-0000-0000-0000-000000000002
  profileName: example-other-profile
  active: true
  revision: 6
```

---

### runtime profiles get

Get a security profile by name or UUID

```text
airs-cli runtime profiles get [options] <nameOrId>
```

#### Arguments

- `nameOrId` (required) —

#### Options

| Flag | Required | Default | Description |
|------|:--------:|---------|-------------|
| `--revision <n>` | No | latest | Select an exact revision |
| `--all-versions` | No | — | Return every matching revision as a list |
| `--output <format>` | No | Resolved | Output format: pretty, table, markdown, csv, json, yaml |

#### Examples

*Pretty output (fallback `pretty`)*

```bash
airs-cli runtime profiles get docs-example-profile
```

```text
Prisma AIRS — Runtime Configuration
Security profile and topic management


Profile Detail:

  ID:       00000000-0000-0000-0000-000000000001
  Name:     docs-example-profile
  Status:   active
  Revision: 1
  Created:  user@example.com
  Updated:  user@example.com
  Modified: 2026-05-25T13:38:21Z
  Policy:   {
              "ai-security-profiles": [
                {
                  "model-type": "default",
                  "model-configuration": {
                    "mask-data-in-storage": false,
                    "latency": {
                      "inline-timeout-action": "block",
                      "max-inline-latency": 5
                    },
                    "data-protection": {
                      "data-leak-detection": {
                        "member": null,
                        "action": "",
                        "mask-data-inline": false
                      },
                      "database-security": null
                    },
                    "app-protection": {
                      "default-url-category": {
                        "member": null
                      },
                      "url-detected-action": "block",
                      "malicious-code-protection": {
                        "name": "malicious-code-detection",
                        "action": "block"
                      }
                    },
                    "model-protection": [
                      {
                        "name": "prompt-injection",
                        "action": "block"
                      },
                      {
                        "name": "toxic-content",
                        "action": "high:block, moderate:alert"
                      }
                    ],
                    "agent-protection": [
                      {
                        "name": "agent-security",
                        "action": "block"
                      }
                    ]
                  }
                }
              ]
            }
```

*JSON output (flattens management response — `profileId` / `profileName` keys)*

```bash
airs-cli runtime profiles get docs-example-profile --output json
```

```text
{
  "profileId": "00000000-0000-0000-0000-000000000001",
  "profileName": "docs-example-profile",
  "revision": 1,
  "active": true,
  "createdBy": "user@example.com",
  "updatedBy": "user@example.com",
  "lastModifiedTs": "2026-05-25T13:38:21Z",
  "policy": {
    "ai-security-profiles": [
      {
        "model-type": "default",
        "model-configuration": {
          "mask-data-in-storage": false,
          "latency": {
            "inline-timeout-action": "block",
            "max-inline-latency": 5
          },
          "data-protection": {
            "data-leak-detection": {
              "member": null,
              "action": "",
              "mask-data-inline": false
            },
            "database-security": null
          },
          "app-protection": {
            "default-url-category": {
              "member": null
            },
            "url-detected-action": "block",
            "malicious-code-protection": {
              "name": "malicious-code-detection",
              "action": "block"
            }
          },
          "model-protection": [
            {
              "name": "prompt-injection",
              "action": "block"
            },
            {
              "name": "toxic-content",
              "action": "high:block, moderate:alert"
            }
          ],
          "agent-protection": [
            {
              "name": "agent-security",
              "action": "block"
            }
          ]
        }
      }
    ]
  }
}
```

*YAML output (the nested `policy` is emitted as inline JSON, not converted to YAML)*

```bash
airs-cli runtime profiles get docs-example-profile --output yaml
```

```text
profileId: 00000000-0000-0000-0000-000000000001
profileName: docs-example-profile
revision: 1
active: true
createdBy: user@example.com
updatedBy: user@example.com
lastModifiedTs: 2026-05-25T13:38:21Z
policy: {
  "ai-security-profiles": [
    {
      "model-type": "default",
      "model-configuration": {
        "mask-data-in-storage": false,
        "latency": {
          "inline-timeout-action": "block",
          "max-inline-latency": 5
        },
        "data-protection": {
          "data-leak-detection": {
            "member": null,
            "action": "",
            "mask-data-inline": false
          },
          "database-security": null
        },
        "app-protection": {
          "default-url-category": {
            "member": null
          },
          "url-detected-action": "block",
          "malicious-code-protection": {
            "name": "malicious-code-detection",
            "action": "block"
          }
        },
        "model-protection": [
          {
            "name": "prompt-injection",
            "action": "block"
          },
          {
            "name": "toxic-content",
            "action": "high:block, moderate:alert"
          }
        ],
        "agent-protection": [
          {
            "name": "agent-security",
            "action": "block"
          }
        ]
      }
    }
  ]
}
```

---

### runtime profiles create

Create a new security profile

```text
airs-cli runtime profiles create [options]
```

#### Options

| Flag | Required | Default | Description |
|------|:--------:|---------|-------------|
| `--name <name>` | Unless `--config` | — | Profile name |
| `--no-active` | No | — | Create profile as inactive |
| `--direction <direction>` | No | — | Protection direction; mandatory for directional protection updates |
| `--no-mask-data-inline` | No | — | Explicitly disable inline masking |
| `--no-mask-data-in-storage` | No | — | Explicitly disable shared storage masking |
| `--enable-full-conversation-inspection` | No | — | Enable shared conversation inspection |
| `--no-enable-full-conversation-inspection` | No | — | Explicitly disable shared conversation inspection |
| `--output <format>` | No | Resolved | pretty, json, yaml |
| `--prompt-injection <action>` | No | — | Prompt injection action (block/allow/alert) |
| `--toxic-content <action>` | No | — | Toxic content action (e.g. "high:block, moderate:block") |
| `--contextual-grounding <action>` | No | — | Contextual grounding action (block/allow/alert) |
| `--malicious-code <action>` | No | — | Malicious code protection action (block/allow/alert) |
| `--url-action <action>` | No | — | URL detected action (block/allow/alert) |
| `--allow-url-categories <list>` | No | — | Comma-separated URL categories to allow |
| `--block-url-categories <list>` | No | — | Comma-separated URL categories to block |
| `--alert-url-categories <list>` | No | — | Comma-separated URL categories to alert |
| `--agent-security <action>` | No | — | Agent security action (block/allow/alert) |
| `--dlp-action <action>` | No | — | Data leak detection action (block/allow/alert) |
| `--dlp-profiles <list>` | No | — | Comma-separated DLP profile names |
| `--mask-data-inline` | No | — | Mask detected data inline |
| `--db-security-create <action>` | No | — | Database create action (block/allow/alert) |
| `--db-security-read <action>` | No | — | Database read action (block/allow/alert) |
| `--db-security-update <action>` | No | — | Database update action (block/allow/alert) |
| `--db-security-delete <action>` | No | — | Database delete action (block/allow/alert) |
| `--inline-timeout-action <action>` | No | — | Inline timeout action (block/allow) |
| `--max-inline-latency <n>` | No | — | Max inline latency in seconds |
| `--mask-data-in-storage` | No | — | Mask data in storage |
| `--config <path>` | No | — | Complete JSON configuration; exclusive with write flags |

#### Examples

*Create with protection flags (no JSON output flag — pretty only)*

```bash
airs-cli runtime profiles create \
  --name docs-example-profile \
  --prompt-injection block \
  --toxic-content "high:block, moderate:alert" \
  --malicious-code block \
  --agent-security block \
  --url-action block
```

```text
Prisma AIRS — Runtime Configuration
Security profile and topic management

Profile created: 00000000-0000-0000-0000-000000000001


Profile Detail:

  ID:       00000000-0000-0000-0000-000000000001
  Name:     docs-example-profile
  Status:   active
  Revision: 1
  Created:  user@example.com
  Updated:  user@example.com
  Modified: 2026-05-25T13:38:21Z
  Policy:   {
              "ai-security-profiles": [
                {
                  "model-type": "default",
                  "model-configuration": {
                    "mask-data-in-storage": false,
                    "latency": {
                      "inline-timeout-action": "block",
                      "max-inline-latency": 5
                    },
                    "data-protection": {
                      "data-leak-detection": {
                        "member": null,
                        "action": "",
                        "mask-data-inline": false
                      },
                      "database-security": null
                    },
                    "app-protection": {
                      "default-url-category": {
                        "member": null
                      },
                      "url-detected-action": "block",
                      "malicious-code-protection": {
                        "name": "malicious-code-detection",
                        "action": "block"
                      }
                    },
                    "model-protection": [
                      {
                        "name": "prompt-injection",
                        "action": "block"
                      },
                      {
                        "name": "toxic-content",
                        "action": "high:block, moderate:alert"
                      }
                    ],
                    "agent-protection": [
                      {
                        "name": "agent-security",
                        "action": "block"
                      }
                    ]
                  }
                }
              ]
            }
```

---

### runtime profiles update

Update a security profile by name or UUID

```text
airs-cli runtime profiles update [options] <nameOrId>
```

#### Arguments

- `nameOrId` (required) —

#### Options

| Flag | Required | Default | Description |
|------|:--------:|---------|-------------|
| `--name <name>` | No | — | Update profile name |
| `--no-active` | No | — | Set profile as inactive |
| `--direction <direction>` | No | — | Protection direction; mandatory for directional protection updates |
| `--no-mask-data-inline` | No | — | Explicitly disable inline masking |
| `--no-mask-data-in-storage` | No | — | Explicitly disable shared storage masking |
| `--enable-full-conversation-inspection` | No | — | Enable shared conversation inspection |
| `--no-enable-full-conversation-inspection` | No | — | Explicitly disable shared conversation inspection |
| `--output <format>` | No | Resolved | pretty, json, yaml |
| `--ai-profile-index <n>` | No | — | Select existing AI entry, zero-based; required if multiple entries exist |
| `--active` | No | — | Set profile as active |
| `--prompt-injection <action>` | No | — | Prompt injection action (block/allow/alert) |
| `--toxic-content <action>` | No | — | Toxic content action (e.g. "high:block, moderate:block") |
| `--contextual-grounding <action>` | No | — | Contextual grounding action (block/allow/alert) |
| `--malicious-code <action>` | No | — | Malicious code protection action (block/allow/alert) |
| `--url-action <action>` | No | — | URL detected action (block/allow/alert) |
| `--allow-url-categories <list>` | No | — | Comma-separated URL categories to allow |
| `--block-url-categories <list>` | No | — | Comma-separated URL categories to block |
| `--alert-url-categories <list>` | No | — | Comma-separated URL categories to alert |
| `--agent-security <action>` | No | — | Agent security action (block/allow/alert) |
| `--dlp-action <action>` | No | — | Data leak detection action (block/allow/alert) |
| `--dlp-profiles <list>` | No | — | Comma-separated DLP profile names |
| `--mask-data-inline` | No | — | Mask detected data inline |
| `--db-security-create <action>` | No | — | Database create action (block/allow/alert) |
| `--db-security-read <action>` | No | — | Database read action (block/allow/alert) |
| `--db-security-update <action>` | No | — | Database update action (block/allow/alert) |
| `--db-security-delete <action>` | No | — | Database delete action (block/allow/alert) |
| `--inline-timeout-action <action>` | No | — | Inline timeout action (block/allow) |
| `--max-inline-latency <n>` | No | — | Max inline latency in seconds |
| `--mask-data-in-storage` | No | — | Mask data in storage |
| `--config <path>` | No | — | Complete JSON replacement; exclusive with write flags |

#### Examples

*Read-modify-write — only the flags you pass change; existing protections are preserved. New revision id returned.*

```bash
airs-cli runtime profiles update docs-example-profile \
  --prompt-injection alert
```

```text
Prisma AIRS — Runtime Configuration
Security profile and topic management

Profile updated: 00000000-0000-0000-0000-000000000002


Profile Detail:

  ID:       00000000-0000-0000-0000-000000000002
  Name:     docs-example-profile
  Status:   active
  Revision: 2
  Created:  user@example.com
  Updated:  none
  Modified: 2026-05-25T13:39:05Z
  Policy:   {
              "ai-security-profiles": [
                {
                  "model-type": "default",
                  "model-configuration": {
                    "mask-data-in-storage": false,
                    "latency": {
                      "inline-timeout-action": "block",
                      "max-inline-latency": 5
                    },
                    "data-protection": {
                      "data-leak-detection": {
                        "member": null,
                        "action": "",
                        "mask-data-inline": false
                      },
                      "database-security": null
                    },
                    "app-protection": {
                      "default-url-category": {
                        "member": null
                      },
                      "url-detected-action": "block",
                      "malicious-code-protection": {
                        "name": "malicious-code-detection",
                        "action": "block"
                      }
                    },
                    "model-protection": [
                      {
                        "name": "prompt-injection",
                        "action": "alert"
                      },
                      {
                        "name": "toxic-content",
                        "action": "high:block, moderate:alert"
                      }
                    ],
                    "agent-protection": [
                      {
                        "name": "agent-security",
                        "action": "block"
                      }
                    ]
                  }
                }
              ]
            }
```

---

### runtime profiles delete

Delete a security profile by name or UUID

```text
airs-cli runtime profiles delete [options] <nameOrId>
```

#### Arguments

- `nameOrId` (required) —

#### Options

| Flag | Required | Default | Description |
|------|:--------:|---------|-------------|
| `--force` | No | — | Force delete (removes from referencing policies) |
| `--updated-by <email>` | No | — | Email of user performing force deletion |

#### Examples

:::warning[Example needed]
No curated input/output example for this command yet.
:::

---

### runtime profiles cleanup

Delete old profile revisions, keeping only the latest per name

```text
airs-cli runtime profiles cleanup [options]
```

#### Options

| Flag | Required | Default | Description |
|------|:--------:|---------|-------------|
| `--force` | No | — | Skip confirmation — proceed with deletion |
| `--updated-by <email>` | No | — | Email for deletion audit (default: git user.email) |
| `--output <format>` | No | `pretty` | Output format: pretty or json |

#### Examples

*Dry run (preview)*

```bash
airs-cli runtime profiles cleanup
```

*Delete old revisions*

```bash
airs-cli runtime profiles cleanup --force
```

*Specify email for audit trail*

```bash
airs-cli runtime profiles cleanup --force --updated-by user@example.com
```

*JSON output*

```bash
airs-cli runtime profiles cleanup --force --output json
```
