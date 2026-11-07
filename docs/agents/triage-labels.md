# Triage labels

Labels are global in BonBon. Reuse these exact strings; never invent near-duplicates such
as `bugs`, `Bug` or `doc`. Every Card on the Prisma AIRS CLI Board also carries
`prisma-airs-cli`.

## Triage role (exactly one per Card)

| Role | BonBon Label | Meaning |
| --- | --- | --- |
| needs triage | `needs-triage` | The Owner or a maintainer needs to evaluate it |
| needs info | `needs-info` | Waiting for information |
| ready for agent | `ready-for-agent` | Fully specified; an agent may claim and do it |
| ready for human | `ready-for-human` | Needs the Owner or a human (also set `assignee_rule: human`) |
| won't fix | `wontfix` | Will not be actioned |

## Kind (at least one per Card)

| Label | Use for |
| --- | --- |
| `bug` | Something is broken |
| `feature` | New capability |
| `maintenance` | Upkeep, refactors, dependency bumps |
| `docs` | Documentation |
| `testing` | Tests, qualification, verification |
| `research` | Investigation or comparison; no implementation |
| `tooling` | Build, scripts, developer tooling |
| `security` | Security-relevant work |
| `ops` | Deploys, infrastructure, operations |

## Modifiers (optional, any number)

| Label | Use for |
| --- | --- |
| `epic` | A parent Card split into child Cards |
| `decision` | Records or needs an Owner decision |
| `owner-action` | Only the Owner can do it (purchases, accounts, physical access) |
| `upstream` | Tracks or reports an issue in a third-party project |
| `idea` | Optional; authorizes nothing |
| `superseded` | Replaced by another Card (say which in a comment) |

Priority uses the Card's `priority` field (`low|medium|high|urgent`), not Labels.

Cards created before 2026-11-07 also carry legacy status Labels (`validate`, `planned`,
`gated`) and area Labels (`backend`, `frontend`, `storage`, `mcp`, `gateway`, `ci`,
`secrets`, `planning`). Leave them in place; don't add them to new Cards.
