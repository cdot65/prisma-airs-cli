# Issue tracker: BonBon

## Tracker

Work on this project is tracked as **Cards** on the **Prisma AIRS CLI** BonBon Board
(`01a1ab75-35eb-751c-8d38-45f09771aa3b`, category `software`, at
https://bonbon.goshawks.io). GitHub, GitLab and Forgejo Issues are not used for this
project. Pull requests are not a request surface either: a PR delivers a Card, it does not
replace one.

Columns and the Status each maps to: Backlog (`TODO`), Ready (`TODO`), In Progress
(`DOING`), Blocked (`BLOCKED`), Review (`AWAITING_VALIDATION`), Revise (`REVISE`), Done
(`DONE`). Validation is required: completed work goes to Review for Owner Validation.

## Interface: Claude Code only

- **Claude Code is the only interface to BonBon.** It uses the `bonbon` MCP server (the
  AIRS gateway, acting On Behalf Of the Owner) through the `bonbon` skill, and falls back
  to the `bonbon-owner-api` skill (Owner REST API at `https://bonbon.goshawks.io/api`)
  when MCP is unavailable.
- **Other agents (Codex, Copilot, scripts) never connect to BonBon.** They do not add a
  BonBon MCP server to their configuration and do not call the API. When they need a Card
  change, they write the request into their reply or handoff (Card title or id, the
  action, the text), and a Claude Code session applies it.

## Conventions

- **Title prefix:** `[Prisma AIRS CLI] ` followed by an imperative or outcome phrase, for
  example `[Prisma AIRS CLI] M8 — Verify the R2 backup set restore path`. All Cards on the
  Board were retitled to this prefix on 2026-11-07.
- **Label:** every Card carries `prisma-airs-cli`, exactly one triage Label and at least
  one kind Label from [triage-labels.md](triage-labels.md). Legacy status Labels on older
  Cards (`validate`, `planned`, `gated`, `superseded`, `owner-action`) stay as they are.
- **Description (markdown):** purpose; scope and boundaries; prerequisites written out
  (MCP cannot write Dependency edges, so gates live in the Card text and in the project
  plan under `docs/`); acceptance criteria and done-when; source documents as repo paths.
- **Vocabulary:** say "Card", "Label", "Status", "Claim" and "Validation". Never "task",
  "ticket" or "issue" in BonBon text.
- **Privacy:** no secrets, credentials, tokens, device identifiers, private hostnames or
  IPs, or personal or household data in titles, descriptions, comments or Results. Refer
  to private records by repo path.
- **Priority** uses the Card's `priority` field (`low|medium|high|urgent`), not Labels.
- **Assignee:** on Cards you create and will carry out yourself, set
  `assignee_rule: agent:<your client_id>`. Use `kaiban:research` or `kaiban:planner` only
  when handing work to the kaiban-runner. Only the Owner can change the rule on an
  existing Card.

## Operations

| Action | Tool |
| --- | --- |
| Check identity | `whoami` (status must be `approved`; `pending` is read-only, `revoked` means stop) |
| Find or read | `search_cards` (`board_id` above, `label: prisma-airs-cli`), then `get_card` |
| Create | search for duplicates, then `create_card` |
| Comment | `comment` |
| Labels | `add_label` / `remove_label` |
| Work a Card | `claim_card` → `heartbeat` (at least every 20 minutes) → `complete_card` |
| Give up | `release_card` with an honest reason |

Working a Card: read the whole Card with `get_card` first, including Owner **Feedback:**
comments on Cards in Revise. Claim before doing any of the work. Post progress with
`comment` on long work. Finish with `complete_card`: a markdown Result (what was done,
what to check, follow-ups) and http(s) links; name repo paths and commit hashes in the
Result when there is no URL.

## Limits

- Agents never set `DONE` or `VALIDATED`; only the Owner validates.
- Don't take over other agents' Claims, and don't let your own Claims lapse.
- Don't archive or rewrite Cards you didn't create unless the Owner asks.
- Don't claim historical work just to mark it complete; comment the observed status.
- MCP cannot create Boards or move Cards between Boards; those go through the Owner (or
  the `bonbon-owner-api` skill on the Owner's explicit request).
- **Board activity alone authorizes nothing:** no implementation, deploys, device changes,
  commits, pushes or messages to other people.
