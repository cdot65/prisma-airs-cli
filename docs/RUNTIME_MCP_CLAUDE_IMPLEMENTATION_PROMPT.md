# Claude Code implementation prompt for the optional runtime MCP service

Copy this prompt into Claude Code with the working directory set to
`/home/cdot/development/cdot65/prisma-airs-cli`.

---

You are the implementation owner for the optional Prisma AIRS runtime MCP service.
Implement the agreed design in this repository, in reviewable phases, preserving
the existing standalone CLI. The decisions below are settled; do not restart the
scope interview or ask permission for ordinary code, test, or documentation work.

## Read these sources first

1. `AGENTS.md` — binding repository rules and the current **9.1/10** acceptance threshold.
2. `docs/RUNTIME_MCP_IMPLEMENTATION_PLAN.md` — authoritative MCP architecture and
   M0–M8 delivery contract. Use the current revision and confirmed packaging choice.
3. `docs/RUNTIME_MCP_PLAN_REVIEW.md` — actual independent Claude Code findings,
   corrections and acceptance. The architecture reached **9.2/10**; acceptance of
   a plan is not acceptance of code, deployment, or live integration.
4. Relevant sections of `docs/DESIGN.md`, plus `docs/COMPATIBILITY.md`,
   `docs/RUNTIME_CREDENTIAL_CONTRACT.md`, `docs/RUNTIME_HISTORY_STORAGE_CONTRACT.md`,
   and `docs/RUNTIME_HISTORY_B4_B5_PRE_REVIEW.md`.
5. `docs/RUNTIME_HISTORY_PHASE_REVIEWS.md` and the B3 qualification/review records
   for the existing source and verification boundaries.

The old `docs/RUNTIME_HISTORY_IMPLEMENTATION_PLAN.md` supplies retained product
requirements. Its local-SQLite CLI delivery, database-backed default report, and
systemd/launchd scheduling approach are superseded by the MCP plan. Do not resume
the old B4 implementation as though this architecture decision had not happened.

## Objective and agreed boundaries

Create a **separate MCP server crate and binary in the existing Rust workspace**,
with its own container and Kubernetes deployment. Move all persistent runtime
database operations into server-owned code. The server owns collection, history,
database lifecycle, durable jobs, schedules, historical reports, and artifacts.

The service is **completely optional**:

- Without explicit MCP selection, `airs` works independently. Preserve direct
  scan/management commands, existing live `airs runtime report`, and offline
  `--from-evidence` rendering.
- With explicit MCP selection, the CLI invokes the service through the AI gateway.
  A failed MCP request must fail clearly; never fall back to live collection.
- The CLI dependency graph must contain neither `rusqlite`/`libsqlite3-sys` nor
  server/history-store code. It must not open or initialize a local history database.
- The existing scan-logs/dashboard API commands keep their direct behavior.
- Persistent local bulk checkpoints and private OAuth session/operation-receipt
  files are distinct from a history database. Preserve their required semantics.

Use these package boundaries:

| Package | Purpose |
| --- | --- |
| `airs-cli` | Standalone CLI and explicit MCP adapters. |
| `airs-runtime-core` | Shared SCM collector/client construction, credential validators, identities/evidence, privacy, analysis and rendering; no SQLite/listener. |
| `airs-history-store` | Server-owned SQLite, reconciliation, retry state, transfers and lifecycle. |
| `airs-mcp-client` | Gateway OAuth, MCP client, typed requests and bounded artifact transfers; no database. |
| `airs-mcp` | Protected MCP server, configuration, jobs, schedules, workers and artifact orchestration. |

**Packaging is decided:** `airs-runtime-core` and `airs-mcp-client` will publish
in lockstep with `airs-cli`, with exact versions. History-store and server remain
workspace-only (`publish = false`). Check crate names and package builds in M0;
do not reserve names or publish as part of implementation. Keep the CLI as the
workspace default member, and qualify both CLI-only and unified feature graphs.

## Deployment, identity and permissions

The request path is:

`Claude Code or airs → Goshawks AI gateway → airs-mcp in namespace goshawks`.

Caller authentication uses the existing Idira/Goshawks Entra delegation pattern.
The gateway-to-backend hop uses OAuth 2.1 and an Entra app registration. Start
authenticated; the earlier no-auth idea is canceled. Do not expose a bypass route.

Use Bonbon as a reference, inspecting its actual code and qualified deployment
behavior. Its upstream Entra token can represent the integration owner, while
the gateway-signed assertion identifies the caller and agent. Never assume those
identities are interchangeable or that “OBO” establishes a particular grant on
every hop. Require the plan's issuer/audience/expiry/tenant/gateway, signed backend
binding, caller/agent/workspace association and per-invocation authorization.

Reference source is available under:

- `/home/cdot/development/redtail/bonbonboard/docs/adr/0004-gateway-sole-mcp-client.md`
- `/home/cdot/development/redtail/bonbonboard/docs/adr/0005-agent-identity-via-identity-assertion.md`
- `/home/cdot/development/redtail/bonbonboard/crates/auth/` and `crates/mcp/`
- `/home/cdot/development/talos-cluster/bonbon/`, `goshawks/`, `aigw-goshawks/`

Read each repository's instructions before working there. Source notes are not
proof of current live settings. Do not print credentials, integration secrets,
tokens, authorization codes or private report data while researching.

Enforce Reader, Operator and Administrator permissions in the server, intersected
with explicit tenant/caller/agent grants and allowed tools. Unknown agents are
denied. Known Claude Code agents default to Reader; grant Operator operations
explicitly. Administrative operations are not available to agents by default.

Follow the accepted administrative-authority contract exactly. Distinct client
IDs or 0600 files under the same OS user do not isolate credentials from a coding
agent. Use an administrative context beyond the agent's privileges, or qualify
fresh, one-operation authorization that cached credentials cannot satisfy. Under
the fresh option, this applies to **every mutation under that identity**, including
direct execution, maintenance and alternate lower-tier paths. Test shell/CLI
bypass, recent cached access tokens, noninteractive confirmation, stale proof,
parameter changes and replay. Without a qualified option, do not provision effective
administrative mutation grants in an agent-accessible context.

The server holds its own AIRS management credentials and application-bound scanner
keys, delivered through the cluster's existing secret-management pattern into
validated TOML. Do not forward local CLI AIRS credentials. Preserve the two separate
credential planes, exact application bindings and secret-safety rules.

## Storage, jobs and collection

Start with **one AIRS tenant/TSG**, with explicit tenant scoping throughout the
model. Use **one active server instance**, SQLite on a filesystem backed by
persistent block storage, private files, tested backups and restore. Restart and
upgrade downtime is acceptable. No SQLite on RWX/NFS; prove single-writer ownership
and fencing rather than assuming RWO prevents multiple pods.

Separate portable history from service control state. Jobs, idempotency, reservations,
cooldowns, denial revisions, tombstones, schedules and publication fences must survive
retries and ordinary restarts. Restoring an old archive cannot reactivate jobs or
reset API allowances. Preserve the plan's conservative control-store recovery,
epoch/quarantine, guarded startup repair and reachable authenticated maintenance.
Never erase or recreate a damaged database to make startup succeed.

Long-running operations return a durable job ID promptly and continue if the
client disconnects or its token expires. Retry the same submission idempotently;
changed parameters under the same key are a conflict. Cancellation is explicit.
Persist checkpoints and API-send reservations before effects, and keep uncertain
sends consumed. Never claim exactly-once external API execution.

Collect scan-log metadata **every 15 minutes**, configurable. Use valid per-source
progress, a bounded overlap and the narrowest supported rolling selector. Coalesce
missed ticks and expose partial/falling-behind coverage. Do not re-traverse 24 hours
unconditionally on every tick. Routine polling makes **zero Reports API calls**.

Renew expired management OAuth credentials and permit one accounted stale-token
401 replay at the failed-read boundary. Do not add whole-job retry loops or stacked
SDK retries. Treat a second 401/permanent denial as credential-blocked; token-endpoint
429/5xx/transient failures use bounded retry/cooldown. Scanner keys need compatible
rotation, not OAuth refresh. Unrelated configuration changes cannot clear denials.

Keep Reports API use sparse:

- Explicit enrichment only; scheduled enrichment is off until enabled.
- Five selected report IDs by default, ten maximum and at most two actual report
  sends per logical job across all credentials.
- Persist the service-wide daily allowance and cooldown across jobs/restarts.
- Reuse cached snippets, stop on 429, and never probe multiple keys for ownership.
- Reauthentication, rotation, a new job ID or restoring history cannot reset quotas.
- Follow the reviewed plan's exact daily defaults, hard ceilings and admission-day
  accounting. The service does not control separate standalone CLI traffic.

Replace unpublished local receipt retention with explicit `--record-history`
MCP opt-in. Preserve scan results/checkpoints even when retention fails; warn once,
never resubmit scans, and never add AIRS calls just to store a result. Use a
non-administrative connection for this noninteractive Operator mutation. Provide
the explicit bulk-checkpoint metadata upload path described in the plan.

Remote receipts are **client assertions**, not provider proof. Complete identity
fields do not elevate their trust. Require server-observed corroboration before
using facts in authoritative counts/verdicts, and prevent forged receipt conflicts
from demoting provider-derived evidence.

## Daily Signal Briefs and retrieval

Schedule daily reporting at **00:00 America/Chicago**, following CST/CDT, for the
previous local calendar day. Use correct half-open UTC bounds for 23/24/25-hour
days. Capture existing evidence without waiting for the concurrent collector;
record scheduled time, actual as-of time and coverage honestly.

Preserve the original report and create immutable changed-evidence revisions
during the following **24 elapsed hours**. Default revision opportunities are
+4, +8, +12, +16, +20 and +24 hours, with the final allowance reserved for the
closing snapshot. Coalesce delays; never move the cutoff. Implement the reviewed
interval/cap rules, including non-dividing intervals and smaller configured
allowances. Revisions, rendering and retrieval make zero Reports API calls.

Keep these artifacts on the server:

- `report.html` — historical Signal Brief with coherent filters, charts and counts.
- `evidence.json` — normalized, sanitized evidence and coverage.
- `review.md` — embedded coding-agent prompt and collection-bound review packet.
- A manifest binding scope, period, job/version, hashes and completeness.

The security-profile matrix has **profiles as rows and detection services as
columns**, with enabled services indicated clearly. Preserve observed profile
history, malicious verdict versus administrator allow/block, monitor-mode context,
unknown/conflict semantics and sample-versus-population distinctions.

Implement the previously planned historical interactions; do not describe them
as already implemented. Preserve full-cohort summaries and documented display
limits. Published artifacts are immutable. Bind publication to deletion fences
so prune cannot be undone by an in-flight renderer or orphan-bundle reconciliation.

**First release is retrieval-only.** Provide small paginated agent-facing Markdown/
summary responses and a separate bounded byte-exact CLI download path. Reauthorize
every access, verify hashes and publish local files privately without overwriting.
Do not put large base64 HTML responses into Claude's context or expose public URLs.
Persist completion events for a future notification layer; do not add senders now.

Retain history and artifacts indefinitely until explicit pruning. Include all
copies, backups and report versions in lifecycle previews; database pruning does
not erase external backups or retained immutable reports. Enforce the reviewed
revision, bundle, queue, manual-report and disk-reservation limits. Pause safely
on low space and report gaps; never silently delete data to recover capacity.

## Delivery phases and acceptance

Implement the plan's chunks in order:

| Phase | Deliverable |
| --- | --- |
| M0 | Contracts, package/feature feasibility, loopback prototypes and the mandatory real integration/storage feasibility gate. |
| M1 | Shared core extraction and removal of database dependencies/operations from the CLI. |
| M2 | Authenticated MCP endpoint, identity binding, permissions, configuration and safe tools. |
| M3 | Durable control state, jobs, idempotency, cancellation, recovery and backups. |
| M4 | Collection, OAuth retry, scanner eligibility, quotas, schedules and receipt ingestion. |
| M5 | Portable import/export, migration, prune, compaction, recovery and lifecycle completion. |
| M6 | Historical reports, midnight/revision scheduling, artifact publication/retrieval and assessments. |
| M7 | Optional CLI OAuth/client commands, downloads, remote receipt opt-in and documentation. |
| M8 | Deployment artifacts, scale/storage/platform qualification, final integration and release preparation. |

M0.6 is a **go/no-go gate before M1**, not something to defer until deployment.
Prepare a protected disposable echo/info backend and concrete manifests/runbook,
with no AIRS credentials or customer history. The user must execute the real
gateway/Entra/Idira/Claude/CLI sign-in and storage feasibility checks and provide
sanitized receipts. Prove negative identity/approval cases, response bounds,
single-owner storage behavior and a small encrypted restore. Prepare everything
reviewable before requesting that step. Loopback tests cannot pass this live gate.

For every implementation phase:

1. Complete the chunks and their documentation/tests.
2. Freeze the exact source/diff and relevant verification receipts.
3. Obtain an **independent Codex review scoring at least 9.1/10**.
4. Correct all material findings and repeat review if the score is lower.
5. Record the score, reviewer, source hashes, test evidence and limitations in
   `docs/RUNTIME_MCP_PHASE_REVIEWS.md`. Preserve failed reviews honestly.
6. Advance only after the phase's actual requirements pass. A high score cannot
   waive failing tests or missing live evidence.

A0–B3 acceptance is historical evidence only. B4 is unfinished and B5/B6 were not
accepted. Requalify extracted/modified behavior; never inherit an old score.

Required checks remain:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
AIRS_REQUIRE_PARITY=1 cargo test --workspace
```

Use Node 24 and the pinned TypeScript reference. No parity skipping, weakening
sandbox guards, modifying the SDK checkout, deleting v7 tags, or agent-run live
AIRS tests. Add the meaningful negative, crash-point, auth, DST, quota, privacy,
receipt-forgery, import/prune and million-scan tests specified by the plan.
Update DESIGN and applicable §12 differences, paired parity tests, COMPATIBILITY,
TESTING, CLI help/docs and release notes with each observable behavior change.

## Working and stopping rules

Work in the existing checkout. First inspect the dirty state and preserve prior
work, private data, frozen RC4 packages and the reviewed harness fixes. Do not
reset the tree, discard unfinished B4 work or overwrite existing test packages.
Keep one merge owner; independent reviewers inspect without implementing the
phase they score. Coordinate any parallel work so it cannot collide in shared files.

Proceed autonomously on authorized implementation and verification. Ask only when
an unresolved decision or an external action actually blocks the next step. Resolve
ordinary engineering details through the reviewed plan, source inspection and
tests. When a real environment prerequisite blocks a phase, leave an exact
handoff: completed artifacts, concrete command/runbook, expected evidence, owner,
and what can proceed independently. Do not claim the gate passed or route around it.

No publishing, signing, tags, production deployment, notification sending, or real
AIRS scans merely because a phase passes. Prepare these actions for their separate
release/deployment step. Preserve the existing CLI distribution channels; supporting
crate publication order and the server image release are distinct deliverables.

Begin with M0. Report progress with completed work, verification and remaining
constraints. Finish with exact changed artifacts, phase review scores, reproducible
test results, and honest integration/release status.
