# Runtime MCP implementation plan review

Status: revision 6 and the Claude Code implementation handoff were independently
accepted by Claude Code at **9.3/10**. The revision 5 architecture previously passed
at 9.2/10; revision 6 records the confirmed packaging option. No MCP
implementation, deployment, or live integration acceptance is claimed.

The reviewed deliverable is
[RUNTIME_MCP_IMPLEMENTATION_PLAN.md](RUNTIME_MCP_IMPLEMENTATION_PLAN.md).
The gate is an honest independent Claude Code score of at least **9.1/10**, with
material findings resolved. Each later M0–M8 implementation phase separately needs
an independent Codex score of at least 9.1 and its executable verification evidence.

## Review method

Claude Code reviews run in fresh noninteractive sessions. The first review used
a curated frozen copy of the plan, repository contracts, relevant Rust source and
Bonbon reference source, with only Read/Glob/Grep tools and restricted access.
Subsequent reviews receive frozen plan text, the prior review and exact changes
with all tools disabled; the final amendment review also receives the handoff
prompt. No session has configured MCP servers or implementation tools. Review
prompts prohibit live API calls, credential/private-report access, changes and
delegation. The current checkout remains available for corrections without
changing reviewed snapshots. Raw results, invocation receipts, prompts and source
hashes are retained under `.local/dev/runtime-history/mcp-plan/`.

Plan-only verification does not certify runtime tests, storage behavior or a live
Gateway/Entra integration. Those remain implementation-phase requirements.

## Revision 1

Frozen plan SHA256:
`833855d3168ca73e1f9a5c92671c39843e8acd11df39f121b55f81c609b63d0e`.

Snapshot: 229 files, including the plan and relevant source. Raw receipt directory:
`.local/dev/runtime-history/mcp-plan/review-1/`.

Claude Code outcome: **8.2/10 — Revise.**

Reviewer: Claude Code 2.1.316, model `claude-opus-5-5`, fresh restricted session.
The successful result records 29 turns and 439,788 ms. Result SHA256: `3eb7eabe185af41088262b44a7b5f22abbce271e32d2a3ed1cb720e258e86f3e`.

# Independent review: `docs/RUNTIME_MCP_IMPLEMENTATION_PLAN.md`, revision 1

**Overall score: 8.2/10. Verdict: Revise.**

The plan is unusually thorough and mostly consistent with the storage and credential contracts. It falls short of 9.1 for three reasons:
- The riskiest integration assumptions (gateway, Entra, identity assertion, CLI OAuth, storage) aren't checked live until M8, after six phases of irreversible restructuring.
- The authorization principal and agent authority model is underspecified, which matters for a security tool that hands attacker-written prompts to an agent.
- Two outward-facing or product decisions are presented as engineering defaults: publishing new crates and scheduled Reports enrichment.

I found no impossible architecture. I reviewed only the frozen snapshot (Read/Glob/Grep). I ran no commands and edited no files.

---

## Blockers (must fix before acceptance)

### H1. The plan says M0 "must prove" things, but nothing proves them live until M8
**Refs:** lines 99–102, 122–124, 259–264, 427–429, 477 (M0 exit gate: "No live integration claim"), 485, 489–490; AGENTS.md (tests are loopback-only).

- Line 122 says M0 must prove the exact resource/scope/audience combination. Line 261 says M0/M8 must prove exclusive volume ownership. But M0's exit gate rules out live evidence, and automated tests are loopback-only. A loopback prototype can't prove Entra's behaviour. Bonbon only learned about AADSTS9010010 empirically (ADR 0004 amendment).
- The plan calls Bonbon's ADRs "accepted" and its flow "qualified". In fact ADR 0004 is "accepted (pending spike, issue 13)", and ADR 0005 says identity forwarding (`JWT_PRIVATE_KEY`) wasn't configured on aigw-goshawks when it was written.
- These all go live for the first time at M8.3, after M1 has removed CLI database ownership and M2–M7 have been built on them:
  - gateway `oauth_auto` to a new backend
  - presence and claims of `X-User-JWT`
  - the CLI as an Idira public client with a loopback redirect and refresh
  - Streamable HTTP and long-call behaviour through the gateway
  - Claude Code's tool-result limits
  - ReadWriteOncePod support in the CSI driver

**Fix:** add **M0.6, a user-run authorized feasibility spike**, as a go/no-go gate before M1.
- Use a throwaway backend with no AIRS credentials and a trivial echo/info tool.
- Record sanitized claim shapes only, never token values:
  - upstream `aud`/`scp`/`roles`/`azp`/`oid`/`tid`
  - assertion `sub`/`client_id`/`workspace_id`/`organisation_id`/`aud`/`exp`
- Check: Claude Code → gateway → backend; CLI loopback PKCE login and refresh against Idira; maximum tool-result size through the gateway and Claude Code; whether RWOP is supported on the installed storage class.
- Reword line 122 as "M0.6 proves…", and keep the loopback prototype as M0.2.

### H2. The authorization principal is undefined
**Refs:** lines 110–115, 128–131, 146, 300–306, 153–156.

- Bonbon's upstream token comes from a one-time Owner consent when the integration is created (ADR 0004, Consequences). So the Entra `oid` may be the *integration owner* for every caller. The *caller* is named by `X-User-JWT.sub` (Idira user) and `client_id` (agent).
- Bonbon's middleware never relates `ia.sub` to `oid` (`references/bonbon/crates/mcp/src/lib.rs:113-130`). Its `IdentityAssertion` treats `sub`/`workspace_id` as optional (`auth/src/lib.rs:342-354`).
- The plan says to "map oid to the Idira subject". It never says which identity is the key for:
  - grants
  - idempotency scoping ("authenticated submitter", line 300)
  - audit actor
  - revocation
- It also doesn't say whether several human users are supported at launch.

**Fix:**
1. Define the principal tuple: Entra `tid` + upstream `oid` (as the integration-owner constraint) + Idira `sub` + agent `client_id` + `workspace_id`/organisation. Any missing element is denied.
2. Key grants, idempotency and audit on `(sub, client_id)` within the tenant.
3. Require the assertion to carry a per-backend audience or integration binding. If the gateway can't provide one, document the cross-backend assertion-replay analysis.
4. Confirm with the user whether launch is single-owner. If it is, say so and simplify.

### H3. Prompt-injected agents can reach destructive tools
**Refs:** lines 133–144, 149–150, 408–413, 422–429.

- AIRS snippets are, by definition, attacker-written prompts. The plan sends them to Claude Code through `artifacts_read` and `review.md`, and the same agent session may hold Operator/Administrator tools (prune, artifact deletion, restore, allowance raise).
- "Preview then execute" doesn't stop an injected agent, and the plan itself admits identity doesn't prove human review (line 143). Server-side "excerpts cannot authorize tools" doesn't address the agent being the confused deputy.

**Fix:**
- Grant authority per agent `client_id`, with defaults: Claude Code agents get Reader + `assessments_submit` (and optionally collection_start). Administrator lifecycle tools are denied to agent principals unless configured explicitly.
- Bind destructive execution to an out-of-band confirmation, such as a code issued by `airs mcp confirm <preview-id>` under a distinct CLI principal.
- Add allow/deny fixtures to M2 and an injection-in-snippet scenario to M6/M7.

### H4. Publishing new crates is an unconfirmed outward-facing decision
**Refs:** lines 70–79, 562–566; `docs/RELEASING.md:12-16, 70-71`; `Cargo.toml:34`.

- If `airs-cli` depends on `airs-runtime-core` and `airs-mcp-client`, those crates must be published to crates.io first, because Homebrew builds from the crates.io crate (RELEASING.md:15-16).
- crates.io publishing is permanent (yank only). It would make the privacy, evidence and rendering internals public API, and compile rmcp and OAuth dependencies into every CLI install.
- Line 78's "exact internal package versions plus paths" assumes this without the user having confirmed it.

**Fix:** put this in the confirmed-decision table before M1.
- Which crates are published (proposal: core and client in lockstep at exact `=` versions; `publish = false` on history-store and mcp).
- Publish order, crate-name availability check, and `cargo package` builds outside the workspace.
- Whether the MCP client is always compiled in or behind a default-on feature, and the resulting binary-size delta.

### H5. Revision storms and unbounded artifact growth
**Refs:** lines 39, 41, 377–385, 408–421, 282–291.

- Any committed evidence change in the 24-hour window can enqueue a revision. With 96 collection ticks plus the enrichment job, one daily report could get ~97 immutable revisions, retained indefinitely and backed up.
- Each revision can be up to the 32 MiB HTML ceiling plus evidence. Coalescing only merges revisions that are still *pending*.
- There's no debounce, no per-report revision cap, no PVC capacity model, and no behaviour defined for low disk space. Compaction needs 2× the database size free (storage contract :328).

**Fix:**
- Debounce revisions: at most one per configurable interval (default a few hours), plus a final revision at window close if the digest changed. Set a hard maximum per report.
- Write a capacity model at the maximum thresholds.
- Add a low-space watermark that pauses publication, then collection, before compaction headroom is lost, exposed through health and metrics.
- Add tests for each.

---

## Medium findings (fix in this revision or bind explicitly to M0)

### M1. The package table leaves the collector and credential code without a home
**Refs:** lines 53–59, 68–74.

- `report/signal/collect.rs:186-251` depends directly on `history::sync::Sync`.
- `history/sync.rs:10-14` depends on `TenantContext`, `ScannerReference`, `SecretSet` and `ReadBudget`.
- `identity.rs` depends on `clients`/`tenant`.

The live CLI report and the server both need the collector, and the server must not depend on the CLI crate. But core is listed with only "collection interfaces" and no `scm` dependency.

**Fix:** M0.1 names concrete homes. The collector, SDK client construction (`clients.rs`, `RecordingProvider`), URL admission, `ReadBudget`, `SecretSet` and the scanner credential model go in core, which depends on scm `=0.2.4`. A `HistorySink` trait is implemented by history-store. The server's config reuses the scanner-schema validators.

### M2. Cargo feature unification can change SDK wire bytes
**Refs:** `Cargo.toml:56-63, 80-81`; plan lines 76–80, 186.

`cargo test --workspace` unifies features across members, and the CLI links `airs-mcp-client` directly. rmcp, schemars, axum or jsonwebtoken could pull in `serde_json/preserve_order`, a second reqwest, or extra tokio features.

**Fix:** add an M0.2 gate:
- `cargo tree -p airs-cli -e features` (and the unified workspace build) shows no rusqlite, no `preserve_order`, and the reqwest pin intact.
- Parity tests pass with the server members in the build.
- Record the CLI binary-size delta.

### M3. Per-tick collection window and management API load are unspecified
**Refs:** lines 356–362.

Nothing says what each 15-minute tick queries. If ticks re-traverse a rolling 24 hours, that's 96 full traversals per day.

**Fix:**
- Define the tick window as the traversed-as-of watermark minus an overlap, within the SDK's minimum rolling granularity.
- Set per-tick request/page ceilings.
- Track daily management-request and scanner Results counts as metrics.
- M4 proves the measured requests per tick.

### M4. Ambiguities in the shared Reports allowance
**Refs:** lines 344–352, 370–375.

- "Local calendar day" isn't explicitly America/Chicago.
- Wall-clock rollback could move a reservation into the previous day; a max-seen-time guard would prevent that.
- An Administrator "raise" has no compiled hard ceiling.
- Default-on daily example enrichment is a *scheduled* Reports use. The confirmed table (lines 37, 39) only exempts collection and revisions from Reports calls, so the user should confirm default-on, or the default should be off.

### M5. Remote receipt retention can silently lose results and weaken trust
**Refs:** lines 221–233.

- A two-second budget per command means large bulk runs record a small fraction of results, with one warning, no spool and no later path. That leaves permanent gaps.
- Client-submitted receipts can create conflicts that demote promotions derived from management data (storage contract :213-216).
- The "admitted server-side association" model is undefined, so receipts may never promote.

**Fix:**
- Make the budget per batch, or add an explicit `receipts upload --from-bulk-state` that reads existing outputs and makes no AIRS calls.
- Define the association: (principal, agent, declared scanner name) → application ID and binding revision.
- Label client-submitted provenance, and limit when it can demote a management-derived promotion.
- Require the local tenant's cloud ID to equal the connection's expected server cloud.

### M6. Control-store restore, idempotency and backups
**Refs:** lines 282–306.

- A restored control store loses idempotency records, so a CLI operation receipt could create a duplicate job. **Fix:** add a control-epoch ID to job IDs and receipts. After a restore, keys from an unknown epoch return a `control_restored` conflict that requires explicit resubmission.
- Backup destination, encryption, cadence/RPO, access control, retention and restore-drill frequency are unspecified, even though backups contain snippets. **Fix:** M0.4 names them; M8.2 runs a drill.

### M7. Artifact chunk sizes don't fit Claude Code
**Refs:** lines 422–429.

- Claude Code's default MCP output limit (`MAX_MCP_OUTPUT_TOKENS`, ~25k tokens) is far below a 256 KiB result. Base64 of 64 KiB already exceeds it.
- A 32 MiB HTML would take 512 reauthorized calls.

**Fix:** split the tools. Agent-facing tools return UTF-8 segments of `review.md`/summaries under a measured cap (~16–24 KiB). Byte-chunk download is the CLI path. Make the ceiling configurable.

### M8. CLI auth, local state and interruption behaviour
**Refs:** lines 170–182, 205–213, 303–306.

Specify:
- Only `airs mcp login` ever opens a browser, even on a TTY.
- Cache and operation-receipt locations, format and cleanup.
- An explicit DESIGN carve-out from the AGENTS rule "readers never write", since a token refresh writes the cache.
- Ctrl-C while waiting on a remote job: exit 130/143, print the job ID, don't cancel, resume with `jobs get`.
- Refusal and rollback snapshot rules for the new tenant-TOML MCP namespace in old binaries, mirroring credential contract §4.
- Removal of `--history-dir`/`--no-history` from scan/bulk/resume (`commands/scan.rs:48-49`), with the matching DESIGN rows.

### M9. Carry-forward accuracy
**Refs:** lines 400–406, 437–442.

- Line 400 says historical HTML "retains" filters. Current Signal HTML is script-free (`signal/html.rs:573-578`, B4/B5 pre-review B5.8), so the filters are new work needing CSP and §12 changes. Reword.
- M5's gate should name the three known red B4 probes (erased app ID admitted, unowned report admitted, array `scan_id` in work metadata; pre-review lines 22–52) and `tests/support/history_transfer_adversarial.rs`.

### M10. Kubernetes volume ownership vs private-path admission
**Refs:** lines 252, 259–264; `history/paths.rs:55-71`.

- Admission requires euid ownership, `mode & 0o077 == 0`, and no group/other write on non-root ancestors.
- A pod `fsGroup` makes the volume setgid/group-writable, which admission refuses. Without one, a fresh ext4 root is root-owned 0755 (plus `lost+found`), so a non-root process can't create its data root.

**Fix:** no `fsGroup`; an init step creates one uid-owned 0700 data root; fixed non-root uid; read-only root filesystem; an M0.4 test with mount-like ancestors.

---

## Suggestions (not blockers)

- Between M1 and M7 the CLI loses its history/sync commands. State that no release is cut from `main` in that window.
- Old-archive migration (lines 461–466) only concerns unpublished development data. Consider descoping it. If kept, the mounted copy will fail uid admission, so copy it into owned staging first.
- `SecretSet` grows with every lease (`clients.rs:330`). In a long-running server, bound it by lease expiry or per job.
- Pin "eligible authentication rejection" to the existing rule: replay on 401 only, never 403 (`writes.rs:3-6`).
- The 256 MiB SQLite allocator limit is process-wide and shared by both databases and concurrent jobs. Size the connection pools accordingly.
- State which policy lives in TOML and which in the control database. A control restore rolls back database-held policy such as the allowance and schedules.
- State that catch-up reports have no revision window.
- The gateway likely requests the same scopes for every user. Consider Entra app roles as the real discriminator alongside server grants.

---

## Legitimate M0 deliverables (not penalized)

- Exact tool names, schemas, limits, pagination and cursor format; mapping from error codes to exit codes.
- rmcp, JWT, timezone and OAuth crate versions on MSRV 1.89, plus the dependency-feature gate (M2).
- Permission and scope identifiers; server and CLI TOML schemas; cache and operation-receipt formats.
- Queue, range and body ceilings; operation deadlines; per-tick budgets; artifact chunk sizes (after measurement).
- Inventory of crash points for control and archive; backup manifest format; requirements for proving single ownership.
- DESIGN/COMPATIBILITY roadmap and fixture inventory.
- If added, the M0.6 live spike above.

## Strengths

- A clean optionality contract: no fallback, no SQLite in the CLI, an explicit `--mcp` selection, and `--from-evidence` stays offline.
- Splitting the control and archive databases stops an old archive from reviving jobs or resetting quotas. Crash windows are handled honestly, without claiming cross-database atomicity.
- The bounded renewal design matches the SDK's invalidation and generation mechanism (`clients.rs:341`) and the existing replay rule.
- The scheduling is precise: half-open bounds, 23/25-hour days, a cutoff measured in elapsed time, digests over evidence only, and catch-up without fetching examples.
- It reads Bonbon critically: the identity binding is stricter than Bonbon's, unknown agents are denied, and nothing falls back to app-only access.
- Prune now covers artifacts and backups, makes no forensic-erasure claim, and blocks resurrection of removed snippets.
- The phase gates have concrete negative tests and rule out inheriting B3's acceptance.

## Parallel source-grounded planning audits

Independent Codex agents `mcp_credential_scope` and `mcp_storage_fit` inspected
the design and current source without implementing it. They identified these
material issues in revision 1:

1. Remote client receipts could be confused with server-observed provider evidence.
2. Transient OAuth token-endpoint failures could incorrectly block credentials.
3. Schedule-disable authority contradicted the agreed Operator role.
4. Restoring control state could release an unknown provider cooldown at midnight.
5. A stale renderer or orphan publication could expose pruned snippets again.
6. A Kubernetes readiness failure could make authenticated maintenance unreachable.

Revision 2 adds explicit client-asserted provenance and server corroboration,
transient/permanent auth classification and independent credential revisions,
consistent Operator authority, authoritative control ledgers/recovery epochs and
quarantine, publication deletion fences, and separate transport/operation readiness.
It also specifies OAuth cache isolation, preservation of unresolved submissions,
actual-admission-day quotas and serialized report version publication.

Both agents re-read revision 2 and reported no remaining material contradiction
within their reviewed areas. Their audits have no implementation acceptance score
and do not substitute for Claude Code's required review of the final plan.


## Revision 3

Frozen plan SHA256: `dd12119812e24ad26bc8fdaed13959e75f921a6be474136d987ebd1329e3bd1c`.

Claude Code result: **9.0/10 — Revise.** The earlier H1–H5 and M1–M10 findings
were resolved. R1 requires administrative approval credentials to be inaccessible
to the agent execution context, or fresh qualified user-mediated authorization.
The packaging question is explicitly accepted as a prerequisite gate, not a plan
defect. Raw receipt directory: `.local/dev/runtime-history/mcp-plan/review-2/`.

# Independent review: Runtime MCP implementation plan, revision 3

**Overall score: 9.0/10**

**Verdict: Revise.** The remaining gap is narrow.

Revision 3 resolves all five prior blockers (H1–H5) and all ten medium findings (M1–M10). One new material finding remains, R1. It undermines the property the plan now claims for the H3 fix. The fix is a few sentences of plan text, but it is a security-boundary defect, so I cannot score 9.1 with it open.

The unresolved packaging question is **not** why I withhold acceptance. The plan gates it adequately (see below).

This review covers only the frozen text supplied. I executed nothing and inspected no live systems.

---

## Resolution of prior blockers

| ID | Status | Basis in revision 3 |
| --- | --- | --- |
| **H1** Live feasibility deferred to M8 | **Resolved** | M0.6 is a user-run, disposable feasibility spike through the actual gateway, Idira and Entra, required before M1. It covers resource, scope and audience; the signed binding; CLI PKCE login, refresh and interruption; the admin approval path; protocol negotiation; response bounds; RWOP or equivalent; fixed-UID subtree admission; and a small encrypted restore. It requires a mismatched-binding denial. The exit gate says "Loopback evidence alone cannot pass." Bonbon's ADR status is now described accurately as reference only. |
| **H2** Principal undefined | **Resolved** | The full tuple is defined, with upstream `oid` as an integration-owner constraint only. Grants, idempotency and audit are keyed on `(Idira sub, agent client_id)` within the service tenant. Delegation is admitted by explicit configuration. Multiple callers are supported. Missing or ambiguous elements deny access. A signed per-backend binding is required, otherwise M0.6 must resolve the design before M1. Unknown agents get no access, which is stricter than Bonbon's pending read-only. |
| **H3** Prompt-injected agents reach destructive tools | **Resolved at the authorization layer; see R1 for the approval channel** | Grants are per agent. Claude Code defaults to Reader. Operator requires an explicit grant plus an allowed-tool list. Administrator is denied to agents by default. Agent-prepared admin actions need a one-use approval receipt that is bound to the action and issued under a separately registered admin CLI identity. Injection fixtures are required. |
| **H4** Crate publishing unconfirmed | **Resolved as a gate** | The proposal is stated, not claimed as approved. M1 is prohibited until the user answers. The M0 exit gate lists "packaging choice resolved". A one-crate preference forces revisiting the boundary rather than quiet adoption. Name checks happen without reservation, and publication is a separate release action. |
| **H5** Revision storms and unbounded growth | **Resolved** | Revisions are debounced to one per 4 h, including the final one: at most 6 plus the original. The interval floor is 1 h, with a compiled maximum of 24. There is a 64 MiB bundle ceiling. The capacity arithmetic checks out (7 × 64 MiB × 365 ≈ 160 GiB; 25 × 64 MiB × 365 ≈ 570 GiB). Reserves are 2× DB size plus two bundles plus transfers plus 64 MiB. Publication pauses before collection. Full-day evidence-change tests are required. |

## Resolution of prior medium findings

| ID | Status | Basis |
| --- | --- | --- |
| **M1** Collector/credential home | Resolved | Core owns `SecretSet`, `RecordingProvider`, URL admission, scanner validators, `ReadBudget` and the collector, on SCM `=0.2.4`. `HistorySink` replaces `history::sync::Sync`. Secret registries are scoped per job. |
| **M2** Feature unification | Resolved | M0 verifies CLI-only and unified graphs: no SQLite, no `preserve_order`, reqwest/SCM behavior pinned, and full parity with server members included. Transitive MSRV is checked. |
| **M3** Tick window and load | Resolved | Tick window is the watermark minus 15 min, using the narrowest selector. Per-tick limits are 120 s, 200 sends and 40 pages. M4 measures sends and daily totals, and forbids 96 unconditional 24 h traversals. |
| **M4** Reports allowance ambiguity | Resolved | The day is America/Chicago. A high-water guard is required. Compiled ceilings are 10 transports and 50 IDs. Enrichment is off by default; Operator enables it and Administrator raises allowances. Cross-midnight double charging is specified. |
| **M5** Receipt loss and trust | Resolved | `receipts upload --from-bulk-state` is added and gaps are disclosed. The association is defined (`sub`, `client_id`, scanner name → binding) with a cloud match required. `client_asserted` provenance is never elevated and cannot demote provider-derived facts. Forgery and ordering tests are required. |
| **M6** Control restore and backups | Resolved | Recovery epochs are defined. Old keys cannot silently resubmit. A 24 h conservative cooldown applies, and unknown attempts stay exhausted. M0.4 names target, encryption, key custody, RPO, retention and drill cadence. Deployment is blocked without an off-volume target. |
| **M7** Chunk size vs Claude Code | Resolved | Agent responses are paginated UTF-8 capped at 16 KiB. The CLI uses binary chunks of up to 64 KiB within a 256 KiB response. A 32 MiB call count is measured, and smaller chunks are the fallback, never public URLs. |
| **M8** CLI auth, state and interruption | Resolved | Only `airs mcp login` opens a browser. State lives under XDG state. The cache write is documented as generated session state. Interruption exits 130/143 and keeps the job. Rollback and older-binary refusal are required. `--history-dir`/`--no-history` are removed with documentation. |
| **M9** Carry-forward accuracy | Resolved | Filters are described as "previously planned, not yet delivered", with CSP and §12 coverage. The M5 gate names the three red probes and `history_transfer_adversarial.rs`. |
| **M10** Volume ownership | Resolved | No `fsGroup`. A constrained init step creates a UID-owned 0700 subtree. Fixed UID and read-only root are required. M0 and M0.6 test the real mount shape. |

---

## Is the packaging gate adequate?

**Yes, for a reviewed implementation plan.** The decision is:

- recorded as a proposal, not as agreed;
- excluded from the confirmed-decisions table;
- blocking in two independent places (the packaging paragraph and the M0 exit gate);
- given a defined consequence if the answer differs.

No publishing is authorized, and the name check is explicitly non-reserving. This is a user prerequisite, not a plan defect, and by itself it would not prevent acceptance of the plan.

Two small tightenings, not required for acceptance:

- When the user answers, move the outcome into the confirmed-decisions table.
- State that any answer other than the proposal produces a plan revision that needs re-review, not just "revisiting the boundary". M0.1's "freeze package" item depends on the answer.

Accepting the plan does not by itself make implementation ready. M0 cannot exit, and M1 cannot start, until the user answers.

---

## Remaining material finding

### R1. The agent can reach the out-of-band approval channel through the local shell

**Problem.** The H3 control assumes the agent cannot act as the administrative CLI identity. In the primary deployment, Claude Code runs on the same workstation as `airs`, with a Bash tool. The plan stores admin CLI OAuth session material in a private cache under the user's state directory. It also lets noninteractive commands proceed with cached sessions; they only refuse to open a browser.

An injected agent could therefore run `airs mcp confirm PREVIEW_ID` itself. The server would see the registered admin client, not the agent's `client_id`. "The original agent cannot issue its own approval" holds at the identity layer but fails in practice. As written, the plan overstates what the separation provides.

**Concrete fix (plan text only):**

1. `airs mcp confirm` never uses a cached session. Each approval performs a fresh interactive authorization for the admin client, and the server verifies authentication recency (for example `auth_time`/`max_age`) before issuing the receipt. Add authentication-recency forwarding through the gateway to M0.6. If the gateway cannot forward it, M0.6 must resolve an equivalent human-presence mechanism before M1.
2. `confirm` refuses unless it is attached to a controlling terminal. It renders the server-side preview (terminal-safe action, tenant, parameter summary, data generation and expiry) and requires a typed confirmation, not a bare ID.
3. Either keep admin-client refresh tokens out of the shared cache, or make admin grants require the fresh-authentication assertion regardless of cache.
4. State the residual risk: an agent that controls the user's desktop or browser session is outside this control.
5. Add tests in M2/M7: a noninteractive `confirm`, a confirm from a cached session, and a stale `auth_time` are all denied.

---

## Non-material findings (fix opportunistically or bind to M0)

- **R2. Maintenance-mode approvals.**
  - Problem: when the control DB cannot open, one-use preview and approval state has no stated home.
  - Fix: persist maintenance previews and receipts in the maintenance operation manifest. Refuse agent-prepared administrative actions entirely in maintenance mode.
- **R3. Operator-tier agent abuse.**
  - Problem: an injected Operator agent can disable schedules, enable enrichment (consuming the bounded daily allowance), or create manual reports until low-space pauses publication.
  - Fix: the default agent allowed-tool list excludes `schedules_set/disable` and enrichment enablement, and M0.1 freezes a per-principal manual-report rate and quota. "Subject to storage quota" is currently undefined.
- **R4. Node-loss fencing timing.**
  - Problem: M0.6 tests RWOP access but defers node-loss attachment and fencing to M8. The single-active-pod architecture depends on it.
  - Fix: either add a disposable node-down or forced-detach observation to M0.6, or record explicitly that a negative M8 result reopens the design.
- **R5. Terminology collision.** "One integration owner controls this checkout" reuses the identity term for a different concept. Rename it, for example to "merge owner".
- **R6. Classifying a second 401.** M0.1 should state that a 401 after the single fresh-token replay is a permanent credential denial (`needs_credentials`), not a transient one.
- **R7. Decision rule for a missing binding.** Before M0.6, write down what evidence counts as resolving the cross-backend replay threat if the gateway cannot sign a per-backend binding. Candidates are a per-integration signing key, a gateway-to-backend mTLS identity, or a short `exp` plus `jti`, combined with NetworkPolicy. This keeps the post-spike re-review decidable.

---

## Remaining prerequisites (not plan defects)

**User decisions and authorizations**

- Answer the supporting-crate packaging question. M1 is blocked until then, and no publication is authorized.
- Authorize and personally execute M0.6: real resource changes, sign-in, and sanitized receipts.
- Provide grant configuration: which agent `client_id`s get which tier and allowed tools, plus the admin CLI client registration.
- Name backup target, key custody, retention and drill ownership (M0.4).

**Environment and integration**

- Identity forwarding on aigw-goshawks. Per ADR 0005, `JWT_PRIVATE_KEY` is not configured today, so it must be enabled via Conjur in its own infrastructure commit.
- Confirm the assertion carries organization and a per-backend binding. ADR 0005 lists only `sub`, `client_id`, `workspace_id` and `scope`. If unavailable, a design re-review is triggered (R7).
- Entra resource/identifier-URI and scope registration that avoids AADSTS9010010, using new IDs rather than Bonbon's.
- `oauth_auto` to a new backend, the CoreDNS rewrite / `TRUSTED_CUSTOM_HOSTS`, and NetworkPolicy and TLS.
- An Idira public-client registration with a loopback redirect and refresh, plus authentication-recency forwarding if R1's fix is adopted.
- Claude Code and gateway measured output limits; the negotiated MCP protocol version.
- RWOP support on the installed CSI driver (Longhorn), or a demonstrated fenced equivalent.
- The cluster architecture for the server image.

**M0 deliverables (legitimate, not penalized)**

- Frozen tool, CLI, config, cache and receipt schemas and limits.
- Pins for MCP, OAuth, JWT and timezone crates on Rust 1.89, with the feature-graph gate.
- Crate-name availability check (non-reserving), out-of-workspace package builds, and the CLI binary-size delta.
- Crash-point inventory, backup manifest format, and PVC sizing from the capacity model.
- DESIGN/COMPATIBILITY roadmap and fixture inventory.

**Path to acceptance.** Fix R1 in plan text, preferably folding in R2–R7, then request a re-review. With R1 closed, I would expect the plan to meet the ≥9.1 threshold. Implementation readiness would still depend on the prerequisites above, chiefly the packaging answer and a passing M0.6.

## Revision 4

Claude Code result: **9.0/10 — Revise.** Exact plan SHA256: `48826851cb109c2640ed93047f989ec39f3a00a53eeb38aa5146aed115df4719`.

Raw receipt directory: `.local/dev/runtime-history/mcp-plan/review-3/`. The tool-free
review emitted literal tool-invocation markup; no such tool was available or executed.
The Markdown review below is extracted from that result; the raw output is preserved.

# Independent review: Runtime MCP implementation plan, revision 4

**Overall score: 9.0/10**

**Verdict: Revise.** The remaining gap is one narrow but material residual of R1, called R1a below. Everything else from the revision 3 review is closed.

I reviewed only the supplied frozen text and diff. I executed nothing and inspected no live systems. The packaging choice remains an open user prerequisite and is not counted against the plan.

---

## R1 resolution: the confirm path is closed, but a sibling path remains open

**Closed for `airs mcp confirm`.** Revision 4 establishes the property I asked for without prescribing a mechanism:

- **Two qualifying options.** Either the admin session is outside the agent's privileges (0600 under the same OS user explicitly does not count), or fresh user-mediated proof is required that a cached session cannot satisfy.
- **Fresh-proof requirements.** Stale or missing proof is rejected. Forwarding must be signed. Admin refresh credentials stay out of agent-readable caches.
- **Mandatory negative tests in M0.6.** Denial is tested from both the agent's MCP identity and its shell/CLI context, including cached-session, noninteractive and stale-proof attempts.
- **Fail-closed default.** Without isolation or a qualified user-presence path, agent-initiated admin execution stays disabled.
- **Honest limits.** A controlling terminal is explicitly not treated as proof. Explicit `login` alone owns the browser. A compromised admin device, browser or account is placed outside the boundary.

There is no contradiction with "only explicit `login` opens a browser": `confirm` reports the requirement and hands the bound preview to an explicit login.

## R1a (material): direct Administrator execution under the fresh-authorization option

**Problem.**

- The fresh-proof requirement governs only *approval receipts for agent-prepared actions*.
- Its cache restriction names only "admin refresh credentials".
- The administrative CLI identity separately holds Administrator grants for human-initiated operations, which "still require explicit preview/execute confirmation".

So, under the fresh option on a single workstation, an admin **access token** can legitimately sit in the agent-readable session cache. One example is a token obtained minutes earlier by a genuine fresh login.

An injected agent can then skip `confirm` entirely. It can run a direct admin preview and execute (prune, restore, allowance change) through its Bash tool against the admin connection, typing any confirmation through a pty. The server sees the admin client's `(sub, client_id)`, the same server-side indistinguishability that R1 described. The M0.6 shell-context denial test is scoped to the approval path, so it would not catch this.

The isolated-session option does not have this gap, because the whole admin session is beyond the agent. The fresh option is the likely single-workstation choice, though, so the gap matters in practice.

**Fix (plan text; mechanism left open):**

1. Under the fresh option, every Administrator **execution** by the admin client requires the same qualified fresh proof, bound to that one operation's preview and consumed once. This covers human-initiated execution, confirm receipts and maintenance repair.
   - The current "approval requires fresh authorization" sentence is the place to state it.
   - Equivalently, the admin client's Administrator grant is effective only when accompanied by such proof.
2. No admin access token or refresh token that could execute a second operation persists in agent-readable storage.
3. The freshness proof is bound to the preview/operation (for example via the login request's nonce/state), so a fresh token cannot be redirected to a different preview within its validity window.
4. Extend the M0.6 and M2/M7 negative tests: the agent's shell invokes direct admin preview/execute with a recently refreshed admin session, and is denied.

With R1a fixed I would expect ≥9.1.

## Other revision 4 changes

| Item | Status |
| --- | --- |
| **R2** Maintenance approvals | Closed. Previews, receipt consumption and epoch changes are in the manifest, and agent-prepared execution is refused in maintenance mode. |
| **R3** Operator abuse | Closed. Allowed-tool defaults exclude schedules, enrichment and allowances. The 4/h per-principal, 24/day and 48 compiled per-tenant quotas are charged at admission, and cancellation cannot regain capacity. |
| **R4** Node-loss fencing | Closed. M0.6 demonstrates concurrent-owner refusal and node loss with no automated force-detach, and an M8 failure reopens the design. |
| **R5** Terminology | Closed ("merge owner"). |
| **R6** Second 401 | Closed. It is a permanent credential denial and moves to `needs_credentials`. |
| **R7** Binding decision rule | Closed, and stricter than suggested. A cryptographic or demonstrated route/token-bound binding with a written replay analysis is required. `exp`/`jti`/NetworkPolicy alone are rejected, and any alternative needs a revision plus review. |
| Revision anchors | Sound. +4…+24 matches the 4 h debounce and the six-revision cap, and the final cutoff is fixed. |
| Traversal progress | Sound and consistent with the repo's fail-closed invariant. Progress is per source, valid traversal only, and a capped tick stays partial/falling-behind. |
| Packaging | Re-review on a one-crate answer is now explicit. It is still correctly blocking M0 exit/M1. |

**Non-material:** the revision anchors are specified only for the 4 h default. M0.1 should state the anchor rule for configured intervals that don't divide 24 h (for example, final anchor always +24, earlier anchors at `k × interval`).

No other contradictions or impossible requirements were found.

---

## Prerequisites (not plan defects)

**User decisions and authorizations**

- Answer the packaging question. It blocks M0 exit/M1. No name reservation or publication is authorized.
- Authorize and personally execute M0.6: the disposable gateway/identity/storage resources, sign-in, and sanitized receipts.
- Choose the R1 option: an isolated admin account/device, or fresh proof. If neither qualifies, agent-initiated admin execution stays disabled, which is an acceptable outcome.
- Supply the agent and admin-client grant configuration and allowed-tool lists.
- M0.4: backup target, encryption, key custody, RPO, retention and drill ownership.

**Environment and integration**

- Identity forwarding on aigw-goshawks with a signed per-backend binding.
- For the fresh option, a signed, forwarded authentication-recency/user-presence claim that the server can verify per operation.
- Entra resource/identifier-URI and scope registration using new IDs.
- An Idira public-client loopback/PKCE/refresh registration.
- Gateway routing, TLS and NetworkPolicy to a new backend.
- The negotiated MCP protocol and measured Claude Code/gateway payload limits.
- RWOP support on the installed Longhorn CSI driver (or a demonstrated fenced equivalent), and its node-loss behavior.
- An off-volume encrypted backup target.
- The cluster architecture for the image.

**Implementation (M0 deliverables)**

- Frozen tool/CLI/config/cache/receipt schemas and limits.
- Dependency pins on Rust 1.89 with the CLI-only/unified feature-graph gate.
- A non-reserving crate-name check, out-of-workspace package builds and the binary-size delta.
- The crash-point inventory, backup manifest and PVC sizing.
- DESIGN/COMPATIBILITY roadmap updates.



## Revision 5

Claude Code result: **9.2/10 — Accepted**, for the plan only.

Frozen plan SHA256: `a9e004f4afd5fc9493def252cff82327eb2d45c0046d371699fb70bf5066df7d`.
Raw receipt directory: `.local/dev/runtime-history/mcp-plan/review-4/`.

# Independent review: Runtime MCP implementation plan, revision 5

**Overall score: 9.2/10**

**Verdict: Accepted** for plan acceptance only. This does not qualify any implementation, deployment or live integration. I reviewed only the supplied revision 5 text, the diff and the prior review. I executed nothing and inspected no live systems.

---

## R1a: closed

Revision 5 meets each of the four fix conditions from the revision 4 review.

| R1a requirement | Revision 5 text | Status |
| --- | --- | --- |
| 1. Every admin-identity execution needs fresh, operation-bound proof | It covers "every mutating execution under the administrative identity". Named cases include direct human-initiated preview/execute, approval issuance, maintenance repair, quota policy and lower-tier mutations. The mutating grant is "ineffective" without the proof. | Met, and broader than asked. Including lower-tier mutations closes the Admin ⊇ Operator side door. |
| 2. No reusable admin token in agent-readable storage | Neither an access token nor a refresh token capable of authorizing another mutation may remain there. | Met. The access-token case that defined R1a is now named explicitly. |
| 3. Proof bound to one operation and not redirectable | The proof binds to the preview/operation, parameters, actor and a server-issued nonce. It is consumed once at durable admission, and retries resolve to the same operation. A recent token, a bare `auth_time`, a pty or a typed phrase is explicitly rejected as a substitute. | Met. |
| 4. Negative tests for the bypass | M0.6, M2 and M7 add tests where the shell skips `confirm` and attempts direct preview/execute or a lower-tier mutation with a recent cached session. The isolated option must pass the same probes. | Met. |

**Consistency checks**

- **Read-only preview.** Creating a read-only preview stays available so the challenge can be prepared, and it grants nothing.
- **Single authority.** "Confirmation cannot mint authority for a second action" removes the double-spend case.
- **Durable resumption.** Checkpoint resumption without new proof is consistent with "accepted jobs survive token expiry". The plan still rechecks grants before each phase or batch and before destructive commits.
- **Maintenance mode.** Proof consumption there is consistent with R2, which keeps receipt consumption in the private manifest when the control database cannot open.
- **Atomic transitions.** The proof → receipt → job transitions are assigned to M0. That is phase work, not a contradiction.

## Non-material clarifications for M0.1

1. **Configuration where neither option qualifies.** The fail-closed sentence disables only *agent-initiated* admin execution. The earlier "must be unavailable … or approval requires fresh authorization" sentence arguably already constrains the admin identity itself. Still, M0.1 should state it directly: an administrative CLI identity with effective mutating grants is provisioned only under a qualified option. Without one, admin mutations run only from an isolated context or stay disabled. Otherwise the R1a shape could reappear through a misconfiguration.

2. **`--record-history` over an admin connection.** Receipt ingestion is an Operator mutation, and that path has no interactive login. Through an admin identity it will be denied and show up as an "unrecorded" warning. That is correct fail-closed behavior, but docs or diagnostics should steer users to a non-admin connection.

3. **Revision anchors.** The new rule is consistent with the rest of the plan:
   - A 1-hour interval gives slots at +1…+23 plus +24, which is 24 slots and matches the compiled 24-revision maximum.
   - A 7-hour interval gives +7, +14, +21 and +24, and the final cutoff stays fixed.
   - The phrase "reserve the last allowance for the final snapshot" should explicitly apply when the configured allowance is smaller than the slot count. This reading is already implied.

No material contradiction remains in the plan as a whole. Findings H1–H5, M1–M10, R1–R7 and R1a are all resolved.

---

## Remaining prerequisites (not plan defects)

**User decisions and authorizations**

- **Packaging.** Answer the lockstep multi-crate versus single-crate question. It blocks M0 exit and M1. A single-crate answer requires a plan revision and re-review. No name reservation or publication is authorized.
- **M0.6.** Authorize and personally execute the disposable gateway, identity and storage spike, including sign-in, with sanitized receipts.
- **R1 option.** Choose either an isolated admin context or the fresh-proof option. If neither qualifies, admin mutation stays disabled or isolated per clarification 1.
- **Grants.** Supply the agent and admin-client grants and the allowed-tool lists.
- **M0.4 backup policy.** Define the backup target, encryption, key custody, RPO, retention and restore-drill ownership.

**Environment**

- **Gateway identity.** aigw-goshawks must forward identity with a signed per-backend binding.
- **Fresh-proof claim.** For the fresh option, the gateway must provide a signed, forwarded proof that carries the server nonce and operation binding, verifiable per operation. If it cannot, that option does not qualify.
- **Registrations.** New Entra resource/identifier-URI and scope registrations, plus an Idira public-client loopback/PKCE/refresh registration.
- **Network.** Gateway routing, TLS and NetworkPolicy to the new backend.
- **Protocol.** The negotiated MCP protocol version and measured Claude Code and gateway payload limits.
- **Storage.** Longhorn RWOP support or a demonstrated fenced equivalent, and its behavior on node loss.
- **Backup and image.** An off-volume encrypted backup target, and the cluster architecture for the image.

**M0 deliverables**

- Frozen tool, CLI, config, cache and receipt schemas and limits, including the proof-consumption, retry and anchor rules.
- Rust 1.89 dependency pins and the CLI-only versus unified feature-graph gate.
- A non-reserving crate-name check, package builds outside the workspace, and the binary-size delta.
- The crash-point inventory, backup manifest and PVC sizing.
- DESIGN and COMPATIBILITY roadmap updates.


## Revision 6 packaging decision

After the revision 5 review started, the user explicitly selected **Two supporting
crates**: core and client publish in lockstep with the CLI; history-store and server
remain workspace-only. Revision 6 changes only the decision status/table to record
that exact proposed option. No publication is authorized. The final independent
amendment and handoff review accepted revision 6 at **9.3/10**; M0.6 and all
implementation-phase gates remain outstanding.


## Final revision 6 and handoff review receipt

Claude Code result: **9.3/10 — Accepted**, for the plan and handoff only.

Frozen plan SHA256: `c1d8cc4fdbcccf4641efd384b9c067ad53005b5492185216317a33fa81ee5ef1`.

Frozen handoff SHA256: `21ad8fed7f75c830bc6b4c8e19d5790c00766e9d9c95108f2e5af95a3f0deed1`.

Raw receipt directory: `.local/dev/runtime-history/mcp-plan/review-5/`.
Reviewer: Claude Code 2.1.316, model `claude-opus-5-5`, fresh tool-free session.
The successful result records 1 turn and 35,427 ms. Result SHA256: `45550c3a137af171147ac892f6c347d1483753395c94057186fc32e3cdb1a540`.

# Independent review: Runtime MCP plan, revision 6, and its handoff

**Score: 9.3/10**

**Verdict: Accepted.** This covers plan and handoff acceptance only. It does not qualify any code, tests, deployment or live integration.

**Limits of this review.** I reviewed only the supplied revision 6 text, the amendment diff, the prior revision 5 review and the handoff. I used no tools and ran nothing. I did not inspect the repository, `AGENTS.md`, `RUNTIME_MCP_PLAN_REVIEW.md`, Bonbon or the cluster. I take the user's packaging answer as stated in the request. I cannot independently confirm it.

---

## Does the packaging answer close the prior prerequisite?

**Yes.** The revision 5 prerequisite was the lockstep multi-crate versus single-crate question. It blocked M0 exit and M1. Revision 6 closes it consistently in three places:

- **Confirmed decisions table.** A new "Supporting crates" row is placed where user-approved decisions belong. It is not in the "engineering defaults" text that follows the table, which correctly makes no approval claim.
- **Workspace boundary.** "Proposed … awaiting the user's answer" becomes "Confirmed packaging". The publish targets are unchanged: core and client with exact `=` versions plus workspace paths, and history-store and server marked `publish = false`.
- **Change control.** The conditional single-crate escape hatch is replaced by "a later change … requires a plan revision and independent review". That is the correct inverse.

**Architecture is unchanged.** The package table, dependency rules, feature-graph gate and M0 checks are all unchanged. The checks are the non-reserving name check, out-of-workspace package builds, publication order (core, client, CLI) and the binary-size delta.

**Publication is still not authorized.** Publication remains a separate release action, and the amendment says so explicitly in both places. This is consistent with the release section and with the rule that no release is cut from intermediate M1–M7 source.

**M0 gate.** The M0 exit criterion "packaging choice resolved" is now satisfied on the decision side. The verification items remain M0 work.

## Does the handoff faithfully scope the accepted plan?

**Yes.** The handoff matches revision 6 on these points:

- packaging, including no name reservation or publishing, and qualification of both the CLI-only and unified graphs
- optionality and no-fallback behavior
- the two-token identity binding
- Reader, Operator and Administrator grants
- the M0.6 go/no-go gate before M1
- quotas, revision anchors, artifacts and phase gates (Codex reviews at 9.1 or higher, recorded honestly)
- stopping rules: no publishing, signing, tags, deployment or senders

The handoff also carries forward all three non-material clarifications from the revision 5 review:

1. **No admin grants without a qualified option.** It says: "Without a qualified option, do not provision effective administrative mutation grants in an agent-accessible context."
2. **Receipt ingestion uses a non-admin connection.** `--record-history` should run over a non-administrative connection.
3. **Smaller configured allowances.** The revision anchor rules explicitly cover allowances smaller than the slot count.

It defers detailed limits to the plan as the authoritative source. That is acceptable for a prompt that tells the implementer to read the plan first.

## Remaining issues

**No material contradictions or omissions found.**

These non-material items can be handled in M0.1 or the review record:

1. **Record this acceptance before using the handoff.** The handoff cites the 9.2 acceptance of revision 5. `RUNTIME_MCP_PLAN_REVIEW.md` should add this revision 6 acceptance before the prompt is used, so the gate the plan cites matches the current revision.
2. **Clarification 1 is stricter in the handoff than in the plan.** The "no effective admin mutation grants without a qualified option" rule appears in the handoff, not in the plan text. M0.1 should freeze it in the contract so the authoritative document is not weaker than the prompt.
3. **Crate-name unavailability.** If the M0 check finds `airs-runtime-core` or `airs-mcp-client` unavailable, M0 should say whether a rename is a ministerial change or a plan revision. The user approved those names, so treat a rename as a small recorded revision rather than silently picking a substitute.
4. **Publishability of core's SCM dependency.** Lockstep publication requires the SCM `=0.2.4` dependency of `airs-runtime-core` to be resolvable from the registry, not from a path or git source. The existing out-of-workspace package-build check covers this. Note it explicitly so a failure is treated as a packaging blocker.
5. **Handoff omits two M0 packaging items.** It does not mention the binary-size delta or recording the publication order. It relies on the plan for both, which is acceptable.

## Prerequisites still outstanding (not plan defects)

These are unchanged from the revision 5 review:

- **M0.6 spike.** The user must authorize and execute the disposable spike, including sign-in, and provide sanitized receipts.
- **Admin option.** Choose either the isolated administrative context or the fresh-proof option.
- **Grants.** Supply the agent and admin-client grants and the allowed-tool lists.
- **M0.4 backup policy.** Define the backup target, encryption, key custody, RPO, retention and restore-drill ownership.
- **Environment.** Gateway signed per-backend binding, Entra and Idira registrations, RWOP or an equivalent fenced setup, and an off-volume backup target.

### Independent handoff consistency check

The separate Codex reviewer `/root/mcp_storage_fit` compared the handoff with
revision 6 and the revision 5 acceptance. It found no material omission or
contradiction. Its sole finding was the stale acceptance header in this ledger,
which is corrected above. This was a read-only documentation audit, not an
implementation-phase review or score.
