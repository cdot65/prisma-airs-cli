# Runtime MCP implementation phase reviews

Ledger for phases M0–M8 of [the MCP plan](RUNTIME_MCP_IMPLEMENTATION_PLAN.md).
Each phase needs an independent Codex review of the frozen source and its
verification receipts, scoring **at least 9.1/10**, with every material finding
corrected. The phase's actual requirements must also pass; a score cannot waive a
failing test or missing live evidence. Failed reviews are preserved here. A0–B3
scores are historical and are not inherited; B4 is unfinished; B5/B6 were not accepted.

## M0 — Contracts, feasibility and the M0.6 gate

Status: **M0.1–M0.5 and M0.6 preparation independently accepted at 9.2/10 (review
7). The M0.6 live gate was executed on 2026-11-05. Combined review 8 scored 8.5/10 (revise;
P1 resolution 3 accepted with conditions). Review 9 scored 9.0/10 (revise) and accepted plan
revision 7 amendments 1–3. Review 10 ACCEPTED the combined M0 record at 9.1/10 and accepted
amendment 4. M1 may start. Production deployment is not qualified; the M8 checks stay open.**
M0 cannot be accepted, and M1 cannot start, until the user runs the
[M0.6 runbook](RUNTIME_MCP_M0_6_RUNBOOK.md) and its receipts pass P1–P9.

Deliverables:

- [Contract](RUNTIME_MCP_CONTRACT.md)
- [Feasibility record](RUNTIME_MCP_M0_FEASIBILITY.md)
- [M0.6 runbook](RUNTIME_MCP_M0_6_RUNBOOK.md)
- `prototypes/runtime-mcp/`: prototype workspace, spike server, spike CLI and `deploy/m0.6/`
- DESIGN §22.8 and COMPATIBILITY §12 (roadmap)
- Scoped dead-code allowances for the unfinished B4 transfer module

Receipts: `.local/dev/runtime-mcp/m0/`. Review records follow.

### M0 review 1 — 6.8/10, revise (preserved)

Reviewer: independent Codex (`codex exec`, codex-cli 0.166.0), fresh read-only
session, run against a frozen snapshot. Manifest SHA256 of `source-manifest.sha256`:
`4f92f3aefa2203a88a3b7368eeae940944933a968c6fd8a9fff2eb4e6b478e55`. Raw result:
`.local/dev/runtime-mcp/m0/review-1/result.md`. Material findings:

1. The token scope tier was not intersected with the grant tier.
2. An Administrator identity could mutate through lower-tier tools without fresh proof.
3. Result-size bounds ignored rmcp's duplicated text content; a page was about 26 KiB.
4. Per-tool parameter/result schemas, the complete CLI/config grammar and some job
   transitions were not frozen.
5. The CLI prototype had no approved-origin or HTTPS enforcement.
6. The session cache was re-read by pathname after descriptor checks, ancestors were
   not checked, and the lock was opened unsafely.
7. JWKS refetch throttling failed when the cache was stale or the source was down.
8. Marker absence was treated as proof of encryption, and the drill did not verify
   restored row contents.
9. Some negative controls did not isolate in-session reauthorization or actor binding,
   and there was no wrong-state or concurrent-refresh coverage.

Minor findings: upstream `nbf` not required; assertion lifetime ceiling of 900 s
versus the documented 300 s; runbook push/echo/teardown gaps; the Tokio 1.53.3 → 1.54.1
lock difference was not disclosed; the snapshot was not buildable.

### M0 round 2 corrections (submitted for review 2)

Each review 1 finding now has code, tests and documentation. Details are in the
[feasibility record](RUNTIME_MCP_M0_FEASIBILITY.md) §3–§4.

- **Authorization:** token tier ∩ grant tier ∩ allowed tools; an administrative
  identity mutates only through proof-bound tools.
- **Result bounds:** bounds apply to the complete result, with adaptive pages
  (largest 15,672 B) and a 175,406 B chunk result.
- **Frozen schemas:** 27 per-tool schemas and examples in
  `airs-runtime-core/schemas/`, with a validator test.
- **Contract:** `expected_epoch` on idempotent calls; approval consumption
  transitions; a complete CLI grammar, production configuration schema and job
  transitions.
- **CLI prototype:** approved-origin/HTTPS trust; descriptor-safe session and lock
  handling.
- **JWKS:** refetch spacing independent of cache freshness.
- **Backups:** a row-verified drill; positive encryption, wrong-key and
  correct-key restore steps.
- **Tests:** in-session reauthorization, a foreign administrator, wrong `state`,
  unapproved origin and concurrent refresh.
- **Minor findings:** fixed.

Verification:

- Prototype tests on Rust 1.89: **33/0**. Prototype fmt and Clippy pass on 1.89.
- Mutations: **14/14 caught**.
- Unified root workspace with the revised prototypes: **3,876 passed, 0 failed,
  5 ignored, 109 groups, no parity skips**. Clippy is clean on 1.99 after a
  lint-only restructuring of one test helper; the test run preceded that change.
- Required root checks, unchanged since round 1 apart from the documentation:
  fmt, Clippy, **3,843/0/5, 98 groups, no parity skips**.

### M0 review 2 — 8.3/10, revise (preserved)

Reviewer: independent Codex (`codex exec`, fresh session; workspace-write sandbox
scoped to the review directory). It ran against a buildable snapshot whose manifest
SHA256 is `fbc46053df15ccf467b41259bb0a17a62b6aaf341d8df0d799f35acdbd0537da`. Raw
result: `.local/dev/runtime-mcp/m0/review-2/result.md`.

The reviewer confirmed review 1 findings 1–3 and 6–9 resolved, and findings 4–5
partially resolved. It built the prototype on Rust 1.89 and ran the 11 core tests.
Its sandbox blocked loopback binds, so it relied on receipts for the other suites.

Material findings:

1. The compiled DTO envelope rejected the required `expected_epoch`.
2. `job.result` was an open object, and the closure check skipped objects without
   `properties`.
3. Approval-path execution and upload admission were not consistently expressible
   under the `admin_proof` rule.
4. Approved-origin checks ran only after rmcp discovery had already sent requests.

Minor findings: invalid TOML in the configuration example; size evidence overstated
as an upper bound; stale 87 KiB and 200 KB-echo expectations; the session-matrix
prose overstated coverage.

### M0 round 3 corrections (submitted for review 3)

- **Envelope:** the compiled decoder enforces `expected_epoch` and the idempotency
  key per tool (`parse_tool_request`, `check_epoch`). A test round-trips every
  frozen example through it.
- **Job results:** `job.result` uses closed variants (collection with zero Reports
  sends, report, enrichment, lifecycle, export, failure), with succeeded, partial
  and failed fixtures. The closure check now covers every object schema.
- **Authority classes:** each tool's class (`grant`, `direct_proof`,
  `direct_proof_or_approval`, `upload_session`) and its rules are frozen. Uploads
  are admitted through an `upload_admit` preview. The executable transition model
  `airs-runtime-core/src/authority.rs` has tests for the valid flows and for
  cross-operation reuse.
- **Discovery:** a request-layer `OriginGuard` refuses unapproved origins for every
  discovery, metadata and token request and never follows redirects. A
  second-origin fixture receives zero requests.
- **Minor findings:** the contract's TOML example is valid (checked with `tomllib`);
  P6 and echo expectations are corrected and pinned by a test; the live-session
  matrix is complete (3 × 3); the size was remeasured (identical).

Verification:

- Prototype: **40/0** on Rust 1.89. fmt passes; Clippy passes on 1.89 and 1.99.
- Mutations: **20/20 caught**.
- Unified root workspace: **3,883 passed, 0 failed, 5 ignored, 109 groups, no
  parity skips**; Clippy clean.
- Required root checks: fmt, Clippy and **3,843/0/5** (98 groups, no parity skips).
  Root source has been unchanged since that run.

### M0 review 3 — 8.8/10, revise (preserved)

Reviewer: independent Codex, fresh session. Snapshot manifest SHA256 is in
`.local/dev/runtime-mcp/m0/review-3/source-manifest.sha256.sha256`. Raw result:
`review-3/result.md`. The reviewer confirmed review 2 findings 1, 2 and 4 resolved
and finding 3 partially resolved, and confirmed that the round-1 resolutions still
hold. Its sandbox could run the 17 core tests but no loopback suites.

Material findings:

1. Idempotent retries in the authority model returned the original job without
   comparing the supplied digest, generation or approval.
2. The upload flow conflicted with the central admin-proof prose. The model had no
   upload-begin idempotency, no finalize/import transitions, and did not reject an
   `upload_admit` preview on the lifecycle execute path.
3. The compiled `ErrorEnvelope` rejected the frozen `credential_status` field.

Minor findings: explicit `null` idempotency fields were accepted on non-idempotent
tools; the spike CLI's general `call` path lacked the client result ceiling.

### M0 round 4 corrections (submitted for review 4)

- **Retries:** idempotent retries in the authority model must repeat the exact
  admitted binding (preview, digest, generation, approval); any change is
  `IdempotencyConflict`.
- **Uploads:** the model now covers the full flow. `upload_begin` is idempotent by
  key. `upload_admit` previews are refused on the execute and approval paths.
  Finalize requires the full length and a matching hash. An `import_finalize`
  preview needs a finalized upload owned by the caller, at most one per upload,
  with its own proof.
- **Contract:** the admin rule is restated by authority class; retry and upload
  lifecycle rules are added (§3.5–§3.7).
- **Errors:** the compiled `ErrorEnvelope` now carries `credential_status`. A test
  round-trips schema-valid errors for all 17 codes, and checks that the compiled and
  frozen code sets are equal.
- **Minor findings:** explicit `null` idempotency fields are refused; the spike CLI
  shares one complete-result bound across `call` and `download`.

Verification:

- Prototype: **42/0** on Rust 1.89. fmt passes; Clippy passes on 1.89 and 1.99.
- Mutations: **25/25 caught**.
- Unified root workspace attempt 1: 3,884 passed and **1 failed**. The failure was
  the existing harness test `lock_holder::tests::hold_takes_the_flock_and_writes_the_record`
  (`EWOULDBLOCK` on lock release). Neither M0 nor this round changed the harness.
  The test passed 8/8 in isolation (`r4/lock-holder-rerun.log`). The probable
  cause, not yet established, is a lock descriptor inherited by a child that a
  parallel test spawned. The log is preserved as `r4/unified-workspace.attempt1.log`.
- Unified root workspace attempt 2: **3,885 passed, 0 failed, 5 ignored, 109 groups,
  no parity skips**; Clippy clean.
- Root required checks: 3,843/0/5; root source unchanged.

### M0 review 4 — 8.9/10, revise (preserved)

Reviewer: independent Codex, fresh session. Snapshot manifest SHA256 of
`source-manifest.sha256`: `5ee6d61a2185371cb3f90f20b84f88d1b40cd70040d5d9d1486ec5b0f45670f7`.
Raw result: `review-4/result.md`. The reviewer confirmed review 3's findings
resolved, apart from the residual below, and confirmed that earlier resolutions
still hold. It ran 18 core tests; its sandbox could not bind loopback sockets.

Material finding: import admission did not recheck the upload or consume it
exclusively at execution time. Its probe showed two overlapping `import_finalize`
previews admitting two jobs for one upload, a preview executing after an abort, and
an expired finalized upload receiving a preview. The round-4 claim of "at most one
import per upload" was therefore overstated.

Minor findings: the error decoder accepted a malformed `job_id` and an explicit null
`credential_status`; a failed download could leave its partial file; §11
configuration prose kept the old "non-proof mutating" wording.

### M0 round 5 corrections (submitted for review 5)

- **Import admission:** the import transition rechecks upload ownership, expiry and
  finalized/unconsumed state, and consumes the upload together with the job insert.
  New tests cover overlapping previews (exactly one job, exact retries still
  resolve), abort after a preview, and expiry.
- **Error decoder:** the compiled decoder validates `job_id` and refuses explicit
  `null` optional members, with differential schema/decoder mutations.
- **Downloads:** a drop guard removes the partial file on every exit, pinned by an
  end-to-end early-failure check.
- **Contract:** §11 grant validation is restated by authority class; §3.7 states
  the execution-time recheck.

Verification:

- Prototype: **44/0** on Rust 1.89. fmt passes; Clippy passes on 1.89 and 1.99.
- Mutations: **28/28 caught**.
- Unified root workspace: **3,887 passed, 0 failed, 5 ignored, 109 groups, no
  parity skips**; Clippy clean.
- Root required checks: 3,843/0/5; root source unchanged.

### M0 review 5 — 8.9/10, revise (preserved)

Reviewer: independent Codex, fresh session. Snapshot manifest SHA256 of
`source-manifest.sha256`: `363e72842173fb452325d7d2ed65a42b5be579ad6126709fd1a329ea1cc32d98`.
Raw result: `review-5/result.md`. The reviewer confirmed all review-4 findings
resolved and confirmed that earlier resolutions still hold.

New material finding: spike MCP sessions were not bound to the authenticated
principal. An in-process probe showed that a different valid, granted principal
could GET (200) and DELETE (202) another principal's session, after which the owner
received 404. Earlier session tests had covered expired, missing and ungranted
credentials, but not a second granted principal. No customer data was involved;
the spike holds only synthetic data.

Minor finding: ID suffixes accepted non-UUID strings, for example 36 hyphens.

### M0 round 6 corrections (submitted for review 6)

- **Session binding:** each spike MCP session is bound at creation to the full
  validated principal tuple. The binding is enforced on POST, GET and DELETE: a
  different valid, granted principal gets 403 and the owner's session is unaffected.
  A refreshed token for the same identity is the same owner. Unknown or ended
  session IDs get the guard's own 404 before reaching the transport. The table is
  bounded.
- **Contract and runbook:** the session rule is in §2.3. Runbook step 7.6 checks
  whether the gateway multiplexes callers onto one backend session.
- **Minor finding:** identifier suffixes are validated as canonical UUIDv4 in both
  the schema (regenerated) and the decoder, with malformed-suffix mutations.

Verification:

- Prototype: **45/0** on Rust 1.89. fmt passes; Clippy passes on 1.89 and 1.99.
- Mutations: **31/31 caught by test failures**. One mutation first produced a
  compile error, which was not counted as a catch. It then survived because rmcp
  also returns 404. The test now pins the guard's own refusal, and the mutation is
  caught.
- Unified root workspace: **3,888 passed, 0 failed, 5 ignored, 109 groups, no
  parity skips**; Clippy clean.
- Root required checks: 3,843/0/5; root source unchanged.

### M0 review 6 — 9.0/10, revise (preserved)

Reviewer: independent Codex, fresh session. Snapshot manifest SHA256 of
`source-manifest.sha256`: `e53c9ff9ef82eba000154a9944dc316089ec1755e20c10f45c38a0b524546aaa`.
Raw result: `review-6/result.md`. The reviewer confirmed the review-5 session
binding resolved, including its own per-field probe, and confirmed that earlier
resolutions still hold.

Material finding: session-capacity admission was not atomic. Its probe started with
9,999 bindings and sent 32 concurrent initializations; the table grew to 10,031,
breaching the documented ceiling.

Minor findings: the `schedules_set` and `schedules_disable` results accepted
arbitrary `schedule_id` strings; the feasibility record listed six server loopback
suites where there are seven.

### M0 round 7 corrections (submitted for review 7)

- **Session capacity:** capacity is reserved atomically. Bound sessions plus
  in-flight creation reservations are checked and reserved under one lock before
  the transport is reached. A reservation becomes the binding under the same lock,
  is released after a failed creation, and is released by a drop guard if the
  request is cancelled.
- **Tests:** with one free slot, 32 requests held open concurrently produce exactly
  one success and 31 503s. The hold uses a loopback-only delay hook, which
  configuration validation refuses off loopback; the deployment template is
  validated in a test. Further tests cover capacity release on delete, no leaked
  reservation after failed creations, and the drop path. Contract §2.3 is updated.
- **Minor findings:** a single shared `schedule_id` definition, referenced by every
  use, with malformed-identifier mutations across nine result identifiers; the
  feasibility breakdown is corrected.

Verification:

- Prototype: **50/0** on Rust 1.89. fmt passes; Clippy passes on 1.89 and 1.99.
- Mutations: **34/34 caught by test failures**. The capacity mutation first
  survived because the concurrent requests did not truly overlap; adding the hold
  made the test exercise the race.
- Unified root workspace: **3,893 passed, 0 failed, 5 ignored, 109 groups, no
  parity skips**; Clippy clean.
- Root required checks: 3,843/0/5; root source unchanged.

### M0 review 7 — 9.2/10, accept-prep (M0.1–M0.5 and M0.6 preparation)

Reviewer: independent Codex, fresh session. Snapshot manifest SHA256 of
`source-manifest.sha256`: `55794a50b96f5a3369c75ff9fbd385525f227beaa7269d8a7f1186ae533d8190`.
Raw result: `review-7/result.md`. No material findings.

The reviewer confirmed review 6's findings resolved and earlier resolutions intact.
Its own in-process probe started with 9,999 bindings and sent 32 concurrent
creations: one success, 31 refusals, final capacity 10,000. It also confirmed that
cancelled and malformed requests release their reservations. It ran 21 core tests
plus focused CLI and session probes. The other suites rely on receipts because its
sandbox cannot bind loopback sockets.

The one minor finding: submission examples named `collection_start` as their job
operation. It was fixed after acceptance (`r8/post-review-7.diff`). The diff
touches only the generator, the regenerated `examples.v1.json` and a new test that
ties each submission example to its operation; `tools.v1.json` and all runtime code
are unchanged. A separate focused independent Codex review of that delta
(`review-7-delta/result.md`) found no findings and returned **DELTA: accept**. It
mutated each operation, reproduced the generated files byte-for-byte and passed 22
core tests on Rust 1.89.

Final verification (`r8/`):

- Prototype: **51 passed, 0 failed** on Rust 1.89. fmt passes; Clippy passes on
  1.89 and 1.99.
- Unified root workspace: **3,894 passed, 0 failed, 5 ignored, 109 groups, no
  parity skips**; Clippy clean.
- Required root checks: `cargo fmt --all --check` passes; `cargo clippy --workspace
  --all-targets -- -D warnings` passes; `AIRS_REQUIRE_PARITY=1 cargo test
  --workspace` gives **3,843 passed, 0 failed, 5 ignored, 98 groups, no parity
  skips**. Root source has been unchanged since that run.

**This acceptance does not pass M0.** M0 needs the user-run
[M0.6 gate](RUNTIME_MCP_M0_6_RUNBOOK.md) with receipts that satisfy P1–P9, followed
by an independent review of the combined record. M1 must not start before then. The
most likely blocker is P1: source inspection found no signed per-backend binding in
the gateway's identity assertion (contract §2.2).

### M0.6 live gate — executed 2026-11-05 (submitted for review 8)

The gate ran against the real Prisma AIRS AI Gateway (2.32.0, `aigw-goshawks`), Entra and
the talos cluster. The agent did the provisioning, deployment and probes; the owner did
the console and browser steps. Sanitized receipts: `.local/dev/runtime-mcp/m0.6/receipts.md`.
Outcome per prerequisite: contract §12.1.

- **P1 is not passed.** The gateway's signed `aud` is the organisation ID, one key signs
  every integration, and custom headers are not forwarded on `oauth_auto`. Proposed
  resolution 3 (upstream-token binding, contract §2.2) awaits independent review 8.
- Step 7.6 found that the gateway multiplexes one backend session per (user, server)
  across agents; the contract now makes M1 stateless (§2.3).
- Step 10.3 (node loss) was not run, by owner decision; it is recorded as a gap.
- Spike changes since review 7: diagnostic denial fields (`ungranted`,
  `assertion_registered`, session `differs`), grant lookup includes the owner `oid`,
  an optional `[route_binding]` mode (unused live), and four new loopback tests.

Verification: prototype **55 passed, 0 failed**, fmt and Clippy clean (Rust 1.89 and
1.99). Required root checks: `cargo fmt --all --check` passes; `cargo clippy --workspace
--all-targets -- -D warnings` passes; `AIRS_REQUIRE_PARITY=1 cargo test --workspace`
gives **3,843 passed, 0 failed, 5 ignored, 98 groups**.

**M0 is not accepted and M1 must not start** until review 8 scores the combined
record at ≥ 9.1 and accepts or replaces resolution 3.

### M0 combined review 8 — 8.5/10, revise (preserved)

Reviewer: independent Codex (`codex exec`), fresh session, against a frozen snapshot.
Manifest SHA256 of `source-manifest.sha256`: 
`753dbb7383966a00f2e8793ba9a924247617d4458af2468f21ead6b57f4c2d88`. Raw result: `.local/dev/runtime-mcp/m0/review-8/result.md`.

**P1 resolution 3 accepted**, with conditions: it is not a cryptographic binding
between the two tokens; the gateway client must be confidential-only and exclusive
to this integration; grants stay owner-aware; regression fixtures must use the
measured org-wide `aud` with no route header; and the plan revision must be explicit.

Material findings:

1. P7 cannot pass without node-loss evidence, and a recorded decision not to run the
   test is not a plan amendment.
2. The stateless remedy was not implemented or qualified through the gateway.
3. The contract disagreed with the frozen schemas (`result_too_large`, `callable_tools`).
4. P8 policy decisions (RPO, retention, drill cadence, owner) were not recorded, and
   the marker-absence requirement had no receipt.

Minor findings: stale status and binding text; image and receipt provenance; logging
documentation said "shapes only" while some identifiers are logged.

### M0 round 9 corrections (submitted for review 9)

1. **P7:** plan revision 7, amendment 3, moves node loss to the M8.2 storage gate on a
   dedicated disposable node. Shared cluster nodes are never powered off, isolated or
   drained for a test. Awaits independent acceptance.
2. **Stateless transport:** implemented in the spike (`[server] sessions = "stateless"`,
   the default), with loopback tests for no issued session, concurrent agents of one
   user, and forwarded-header handling. Deployed as `m06-7` (image `0129354c…`) and
   qualified live through the gateway:
   - reconnect: 15/15 ok;
   - concurrent with the admin CLI: Claude Code 15/15 ok; admin 258 ok and 1
     client-side failure with no server denial; requests interleaved; 0 denials;
   - no `Mcp-Session-Id` ever reached the backend;
   - an ungranted tool was refused live;
   - after a pod restart, Claude Code recovered without reconnecting. One gateway 502
     never reached the new pod; its cause is unverified.

   Plan revision 7, amendment 2, records this.
3. **Schemas:** `result_too_large` added to the generator, the schema, the decoder and
   the tests. `service_info.tools` keeps the frozen schema field, now defined as the
   callable set (tier ∩ grant). Grants that list a tool above their tier are refused
   at load.
4. **P8:** owner-accepted policy recorded: 24 h RPO, the cluster's Longhorn retention,
   a monthly isolated restore drill, the owner accountable. Marker absence was proven
   on a read-only copy of the backup target, as a supplement to the wrong-key and
   correct-key evidence.
5. **P1 conditions:** Entra and gateway checks recorded in receipts; plan revision 7,
   amendment 1; org-wide-`aud` regression fixtures added.
6. **Minors:**
   - status text updated;
   - receipts gain an image-to-window provenance table, with source checksums from
     16:10 onward;
   - logged identifiers are bounded to 128 characters, and the logging allowlist is
     documented in contract §2.2.

Verification: prototype **61 passed, 0 failed**, fmt and Clippy clean (Rust 1.89 and
1.99). Root: fmt and Clippy pass; `AIRS_REQUIRE_PARITY=1 cargo test --workspace` gives
**3,843 passed, 0 failed, 5 ignored, 98 groups** (the root crate is unchanged by M0).

**M0 is not accepted and M1 must not start** until review 9 scores ≥ 9.1.

### M0 combined review 9 — 9.0/10, revise (preserved)

Reviewer: independent Codex (`codex exec`), fresh session, against a frozen snapshot.
Manifest SHA256 of `source-manifest.sha256`:
`2d1373a9cf0b67d45271368ce75f19d89d74e9f63369fb856275f250220e4dd7`. Raw result:
`.local/dev/runtime-mcp/m0/review-9/result.md`.

Plan revision 7 amendments 1 (resolution 3 with conditions), 2 (stateless transport,
qualified live) and 3 (node loss at M8.2 on a dedicated disposable node) are
**accepted**. The schema mismatch and the P8 marker receipt are confirmed fixed.

Material findings:

1. `bounded()` kept DEL and C1 control characters, which JSON does not escape, so a
   hostile identifier could reach the terminal through a diagnostic log line.
2. P8's policy did not name the backup-target access policy, and contract §9.4 still
   called retention a future decision.

Minor findings: an Administrator grant advertised `operator_noop`, which dispatch always
refuses; stale binding and gate text (config.rs, contract §9.3, runbook gate table); the
transport behavior was undocumented; abbreviated source digests, and verification
logs that predated the new tests.

### M0 round 10 corrections (submitted for review 10)

1. **Control characters:** `bounded()` maps every control character (C0, DEL, C1) to
   U+FFFD, as `output::terminal_safe` does. A regression test covers C0, DEL and C1,
   both in the value and in the serialized log line, and checks that the length bound
   counts the original characters. Every other logged field was checked; each is a
   static string or a type name.
2. **P8 policy:** contract §9.4 replaces the proposal with the owner-accepted policy:
   - retention for each copy (nas2 14 daily plus 8 weekly; R2 plus 30 days; nas1
     unverified);
   - the deletion rules;
   - the access roles for write, read, administer/delete and agents.

   Evidence is in the receipts row "P8 access policy". Two unverified facts, nas1's
   retention and the delete scope of the agent-accessible Cloudflare credential, become
   M8.1 deployment checks under the new plan revision 7 **amendment 4**, which needs
   independent acceptance. Contract §12.1 keeps P8's encryption and restore pass
   separate from the policy, which is named but has two open checks.
3. **Minors:**
   - One `callable` predicate (tier ∩ grant, minus the administrative side door) now
     drives `tools/list`, `service_info.principal.tools` and the dispatch refusals. A
     new loopback test is mutation-checked for both listing paths.
   - The stale text in config.rs, auth.rs, contract §2.2 and §9.3 and the runbook gate
     table is fixed.
   - Contract §4.1 documents initialization, ordinary requests, forwarded session IDs,
     GET/DELETE and the retry rule.
   - The receipts list full image digests and source checksums, and new verification
     logs are saved in `verification/`.
4. **Deployed:** spike `m06-8` (image `b41a2b3b…`, source `6d5d8c53…`), with a live smoke
   test through the gateway (receipts row "m06-8 deploy + smoke"). Talos-cluster commit
   `48d1c27`.
5. **Teardown (step 12):** owner decisions recorded. Nothing has been deleted; the `airs`
   integration is kept for M1.

Verification: prototype **63 passed, 0 failed**, fmt and Clippy clean (1.89 and stable).
Root: fmt passes, Clippy exit 0, `AIRS_REQUIRE_PARITY=1 cargo test --workspace` gives
**3,843 passed, 0 failed, 5 ignored, 98 groups**. Logs:
`.local/dev/runtime-mcp/m0.6/verification/`.

**M0 is not accepted and M1 must not start** until review 10 scores ≥ 9.1.

Correction, made after review 10: item 1's claim that "every other logged field" is a
static string or a type name was inaccurate. Review 10 found that the HTTP method is
logged verbatim (HTTP parsing rejects control characters there), and that configured
claim names and configuration revisions are not sanitized (these need a trusted
configuration change).

### M0 combined review 10 — 9.1/10, ACCEPT

Reviewer: independent Codex (`codex exec`), fresh session, against a frozen snapshot.
Manifest SHA256 of `source-manifest.sha256`:
`b378861427e2209aa8521e8bc8ce633b48084d8821c246933e2e39afbfd38239` (665 entries, matched
before and after the review). Raw result: `.local/dev/runtime-mcp/m0/review-10/result.md`.

**M0 is accepted. Plan revision 7 amendments 1–4 are accepted.** M1 may start, under
the existing prohibition on deploying real data during M1–M7. Still open and not
passed: the M8.2 node-loss test on a dedicated disposable node, and the M8.1
backup checks (nas1 retention; no agent-accessible R2 delete right).

Material findings: none.

Minor findings, carried into M1 (the M1 review must check each):

1. Diagnostic hardening: normalize the logged HTTP method to known names or `OTHER`,
   and reject or sanitize control characters in configured claim names and
   revisions.
2. "Callable" means eligible under authorization, not executable: `admin_execute` is
   still listed while admin mode is disabled. Either hide tools that are permanently
   disabled, or report availability separately.
3. Documentation: contract §4.1 must say that in stateless mode the version check
   compares an initialization's header with its own body; it is not checked against
   an earlier initialization. Also remove the stale "proposal" heading in §9.4,
   update the amendment wording in §12.1, and add amendment 4 to the plan's
   exception wording.
4. Provenance: the source checksum method and per-file manifests are now recorded,
   after the review, in receipts and `verification/`. The value the reviewer could not
   reproduce was explained: the build hashed the tree before three deploy-file
   rewrites.

Step 12 teardown was executed after acceptance (receipts row "step 12 teardown executed";
talos-cluster `3d2d58f`, `15d7bca`). The `airs` gateway integration, the encrypted
StorageClass, the key custody and the Entra apps are kept for M1/M8.

## M1 — Extract core and remove CLI database ownership

Status: **ACCEPTED at 9.2/10 by independent review 2** (2026-11-06; review 1 scored
8.7, revise, preserved below).

### M1 implementation (submitted for review 1)

Package boundary (plan "Workspace boundary", contract §1):

- **`crates/airs-runtime-core`** (new; lockstep publication, nothing published). It holds
  the CLI's former `error`, `secrets`, `json`, `debug`, `clients` (URL admission,
  `RecordingProvider`, every SDK client builder), `profile_policy`, `tenant::{config,
  scanners}` with the `Selected`/`TenantContext` value types, `read_budget`, `scan_logs`,
  `withhold` and `withhold_rules`, and `report::signal` (evidence, privacy, analysis,
  detectors, HTML, review, bundle, collector). It also holds the pure helpers they need
  (`text`, `style`, `output_format`, `time`, `limits`, `store`, `files`), split out of the
  CLI modules, and `scan_verdict`, the scan-response normalization from
  `commands/scan.rs`. M1 also moved the M0 skeleton's `dto`, `schedule`, `authority` and
  `schemas/` in from `prototypes/runtime-mcp`. No SQLite, no listener, no CLI I/O. The
  CLI re-exports each module at its former path, so CLI code and behavior are unchanged.
- **Collector seam:** `signal::sink` defines `CollectContext` (session, secrets, payload
  spans, version; built per invocation, never process-global) and the narrow
  `HistorySink` trait (budget, run ID, checkpoint, event limit, session discovery and
  completion). The collector takes `Option<&dyn HistorySink>`; `airs runtime report` and
  `scan-logs` pass none and persist nothing.
- **`crates/airs-history-store`** (new; `publish = false`): the former `src/history`
  (database, repository, identity, paths, inspect, work, transfer, sync, backlog,
  schema). `Sync` implements `HistorySink`, and `sync::run` is the former `runtime sync`
  body as a library operation. `receipts` is the former `retention`, minus its CLI
  adapter (`RecordingApi`, `Invocation`, `Output`): `prepare` takes the admitted archive,
  `job` takes the job's request/scan IDs, and `finish` returns a `Finished` outcome
  instead of printing.
- **CLI:** `runtime history`, `runtime sync`, `--history-dir` and `--no-history` are
  removed (unpublished 8.2-development features; 8.1 never had them). `scan`, `bulk-scan`
  and `resume-poll` no longer retain anything. `rusqlite` is gone from `Cargo.toml`.
  `runtime scan` dropped an endpoint admission that only fed retention:
  `clients::build_scanner` already admits the same endpoint.

Tests:

- Unit tests moved with their modules: core 236 + 9 schema tests; history store 51
  lib tests.
- The executable history tests (`runtime_sync` 21, `runtime_history` 5,
  `runtime_history_import` 10) moved to `crates/airs-history-store/tests/` and now drive
  `airs-history-driver`. That is a test-only binary with the former grammar, JSON
  output, exit codes and signal handling. No release or documented procedure builds or
  installs it, and building the CLI never compiles it; the crate is `publish = false`,
  but a local `cargo install --path` of it remains possible and is unsupported. Two sync tests used to rebind a scanner through `airs tenant scanners set`;
  they now write the same binding with a fresh revision.
- Retired with the feature: the 7 automatic-retention executable tests and the 5
  D345–D349 parity tests. Their replacements:
  - `tests/no_local_history.rs`: the removed grammar is a usage error before any access,
    a scan retains nothing, and the manifest has no SQLite or history store;
  - `receipts_tests::job_scoped_receipts_deduplicate_across_resume_and_never_store_payload`.
- The withholding tests stay in core. Their pretty and table printing assertions, which
  need the CLI renderers, run as `src/gateway/withhold_printed.rs` through core's
  dev-only `test-support` feature, so release builds still carry no checker
  (choice 187).
- The harness now redirects only the driver's `runtime sync` data directory. The CLI's
  scan/bulk/resume tests run against the sandbox's own data directory, so their
  unchanged-sandbox checks prove nothing is written.
- Full accounting: `.local/dev/runtime-mcp/m1/run2/test-accounting.txt`.

Docs:

- DESIGN §11 describes the three packages.
- §12 marks D345–D349 retired. The number is not reused, following the D3/D4/D117
  convention.
- §22 gains a status note and §22.8 marks M1 done.
- COMPATIBILITY §11 rows are retired, and their citations are repointed.
- TESTING, docs-site `runtime.md`, README and contract §1 gate 1 are updated, and the
  command reference is regenerated.

M0 minors carried from review 10:

1. The spike logs a standard method name or `OTHER`
   (`run.rs::logged_method`). Configured revisions, claim names and the proof nonce
   claim must be 1–128 characters with no control character (`config.rs::diagnostic_text`).
   Tests cover both.
2. `admin_execute` is no longer listed while administrative mode is disabled. Called by
   name, it still fails closed with `forbidden` (loopback test).
3. Wording fixed:
   - contract §4.1: the stateless version check;
   - §9.4 heading;
   - §12.1 P7/P8 amendment acceptance;
   - the plan's exception now names amendment 4.
4. The review packet ships a per-file source manifest with the exact command.

Verification (`.local/dev/runtime-mcp/m1/run2/`):

| Check | Result |
|---|---|
| fmt and Clippy (workspace) | clean |
| `AIRS_REQUIRE_PARITY=1 cargo test --workspace` | 3860 passed, 0 failed, 5 ignored (baseline 3843 passed) |
| `cargo tree -p airs-cli` | no rusqlite, libsqlite3, history store, rmcp or axum |
| `preserve_order` | absent from both graphs |
| Single versions | reqwest 0.13.5, scm-sdk 0.2.4, serde_json, tokio, rustls; jiff 0.2.39 is new, as core's contracted dependency |
| `cargo package -p airs-runtime-core -p airs-cli` (outside the workspace) | builds |
| `cargo +1.89.0 check --workspace --all-targets --locked` | passes |
| Prototype workspace | 43 passed |
| Frozen RC4 receipts (`.local/dev/runtime-history/rc4`) | unchanged since the baseline |

Limitation: the binary-size effect of jiff in the CLI graph is not yet measured. M7
measures the client and core delta together.

### M1 review 1 — 8.7/10, revise (preserved)

Reviewer: independent Codex (`.local/dev/runtime-mcp/m1/review-1/result.md`;
manifest `205a33db…`, 630 files, unchanged before and after). Material findings:

1. Core still owned CLI I/O. `debug.rs` created and pruned the CLI's log files and
   wrote warnings to stderr itself, and `style.rs` read the environment and asked
   whether the process streams were terminals.
2. The retention-test migration lost domain coverage: abandoned unflushed receipts, a
   cancelled finish, multi-chunk submissions, and a failed batch under contention.

Minors:

- The core package omitted `LICENSE`/`NOTICE`.
- "Never installed" overstated how the test driver is contained.
- Manifest ordering needs `LC_ALL=C`.
- RC4 preservation lacked artifact hashes.

The review confirmed the package boundary, the narrow `HistorySink`, behavior
preservation (removing the scan admission is neutral), the complete removal, the driver
as a meaningful fixture, `test-support` with choice 187, the four carried M0 minors,
and the byte-for-byte move of the DTOs, authority, schedule and schemas.

### M1 round 2 corrections (submitted for review 2)

1. **CLI I/O back in the CLI.**
   - `src/debug.rs` now owns `DebugPolicy`, `KEEP`, `directory`, `file_name`,
     `log_timestamp`, `prune`, file creation and `open` (stderr as the warning sink),
     together with their tests (`directory_resolution`, `log_names`,
     `retention_keeps_the_newest_twenty_including_this_run`,
     `open_creates_private_files_and_refuses_unsafe_directories`).
   - Core's `DebugLog::with_sinks` is the public constructor over caller sinks. Core
     keeps the record format, sanitizers, token holds, the line check and
     `DebugTransport`.
   - `src/render/style.rs` owns `decide`, `stdout_color`, `stderr_color` and their test.
     Core's `style` is pure `Style`/`paint`.
   - Core now has no filesystem path policy, process-stream access or environment reads
     for presentation.
2. **Receipt domain tests** in `crates/airs-history-store/src/receipts_tests.rs`:
   - `unflushed_receipts_are_abandoned_without_creating_an_archive`: submission, then
     finish without a flush. No archive is created, 2 receipts count as unconfirmed,
     and the run reports one partial outcome.
   - `a_cancelled_run_finishes_interrupted_with_its_flushed_receipts`.
   - `chunked_submissions_keep_every_receipt_through_resume`: 20+20+5 under three
     scan IDs keeps 45 receipts, and the resume adds 45 facts and no receipt.
   - `a_failed_batch_under_contention_counts_exactly_its_receipts`: with the archive
     held busy, exactly the 3 held receipts are unconfirmed, the earlier 2 stay, and
     the run reports one outcome.
3. **Minors:**
   - The core package now includes `LICENSE`/`NOTICE`, held equal to the repository's
     by a test.
   - The driver's containment is stated precisely: building or installing the CLI never
     compiles it, and a local `cargo install --path` of the unpublished crate is
     possible but unsupported.
   - The manifest is generated with `LC_ALL=C`.
   - `run3/rc4-preservation.txt`: `sha256sum -c` of the RC4 packages passes, and no
     RC4 file changed after 2026-11-05.
4. M2 groundwork kept in core: the `history_coverage` parameter types and `utc_ms` in
   `dto.rs`, with their test. No server crate is in this submission; the M2 server is
   developed outside the tree until M1 is accepted.

Verification for round 2 (`.local/dev/runtime-mcp/m1/run3/`):

| Check | Result |
|---|---|
| fmt and Clippy | clean |
| `AIRS_REQUIRE_PARITY=1 cargo test --workspace` | 3866 passed, 0 failed, 5 ignored (accounting in `test-accounting.txt`) |
| `cargo tree` gates | unchanged; `test-support` is absent from the CLI's normal feature graph |
| `cargo +1.89.0 check --workspace --all-targets --locked` | passes |
| Prototype workspace | 43 passed |
| `cargo package -p airs-runtime-core -p airs-cli` | passes from a clean package target; the package lists `LICENSE` and `NOTICE` |

**Packaging correction.** The run2 package receipt was not trustworthy. Cargo
fingerprints registry crates by version, not content, so a package target reused across
runs can verify the CLI against an older core build of the same `8.2.0-dev.1`. Run3's
first attempts showed exactly this. Packaging is qualified only from a clean target
directory, after clearing the overlay extraction.

### M1 review 2 — 9.2/10, ACCEPT

Reviewer: independent Codex (`.local/dev/runtime-mcp/m1/review-2/result.md`; manifest
`6a576ca4…`, 634 files, unchanged before and after, byte-reproducible with
`LC_ALL=C`). Material findings: **none**.

- Both review-1 material findings are resolved:
  - core holds no CLI log-location policy, process-stream access, presentation
    environment reads or terminal detection;
  - `DebugLog::with_sinks` is an adequate injected seam;
  - each of the four receipt tests catches its regression.
- The exit gate is supported: the CLI graphs exclude SQLite, the history store and the
  server, and the normal features exclude `test-support` and `preserve_order`. Receipts
  show 3,866 passed / 0 failed / 5 ignored with no parity skips, the migrated tests
  exercise the extracted code, and RC4 preservation is receipt-backed.
- The packaging correction is sound.
- Local review run: fmt, Clippy and Rust 1.89 pass; 973 tests passed and 1 was ignored.
  65 tests were blocked by the sandbox (loopback binds, UID ancestors); these are not
  code defects.
- Its qualification: core still does caller-selected file reads and bundle
  publication. These are shared artifact helpers, not CLI policy.
- Its one minor finding was the test-accounting counts (ten receipt tests; 3,843 → 3,866
  passed, the +23 explained by a duplicate test name). Corrected in
  `run3/test-accounting.txt` after the review.

## M2 — Protected MCP endpoint

Status: **ACCEPTED at 9.1/10 by independent review 5** (2026-11-06; reviews 1–4 scored
8.0, 8.8, 8.9 and 9.0).

### M2 implementation

`crates/airs-mcp` (`publish = false`) is the production server, grown from the
M0.6-qualified spike. It is in the workspace but not in the CLI's graph.

- **M2.1 Transport, metadata, envelopes, limits.**
  - Stateless Streamable HTTP only: no `Mcp-Session-Id`, GET/DELETE 405, a forwarded
    session ID ignored and logged without its value, JSON responses.
  - The spike's bound-session mode and route-binding option are removed.
  - Unauthenticated RFC 9728 metadata on both paths; 401s point to it.
  - Request bodies ≤ 256 KiB (413).
  - Complete-result bounds come from the catalogue (16 KiB agent, 256 KiB CLI).
  - Strict envelope via core `dto` (unknown fields, versions, tenants and idempotency
    misuse refused before any effect).
- **M2.2 Two-token verification and identity mapping.**
  - The spike's verifier, unchanged in its checks: upstream issuer, audience, tenant,
    `azp`, `scp` and `oid`; assertion issuer, audience, lifetime, workspace and
    organisation; pinned key sources; the JWKS refetch limit.
  - Grants keyed by `(sub, client_id)` with the owner constraint. Effective authority =
    token tier ∩ grant tier ∩ grant tools.
  - The catalogue (`catalogue.rs`) is read from core's frozen `tools.v1.json`.
  - Config validation (contract §11) adds: the catalogue-checked grant tiers and
    Administrator rule, `[service]`, `[airs]` (TSG, D48 endpoints, secret-file form,
    revisions, scanners), `[[receipt_associations]]` and `[limits]` within §6, plus
    diagnostic-text rules for grant identifiers and labels.
- **M2.3 Tenant-isolated read tools and audit.**
  - `service_info` (frozen shape; tools = callable ∩ implemented).
  - `history_status` and `history_coverage` read `airs-history-store` through one
    bounded 2 s read, using `Cloud::from_parts` (the same identity as `from_tenant` for
    the same endpoints) and the new `inspect::coverage_groups` aggregation.
  - Signed cursors bind tool, tenant, principal, epoch and parameters.
  - The served tenant must match.
  - One audit line per call (§2.5 fields only).
  - The dispatch order and the unimplemented-tool refusal are documented in contract
    §4.3.
- **M2.4 Configuration staging, reload, health, maintenance.**
  - The service activates only from a private 0600 copy under `config/`.
  - SIGHUP reloads atomically after complete validation, and refuses changes to the
    listener, storage, tenant or cloud identity. A request is verified and authorized
    against one configuration snapshot, carried in its extensions.
  - Private health listener: `/livez`, `/readyz` (503 in maintenance), fixed bodies.
  - `[service] maintenance` refuses every tool but `service_info`.
  - `control/seed.json` holds the epoch and cursor key until M3.

Tests:

- **`airs-mcp` unit tests:**
  - catalogue;
  - config: the example, grants, service/airs/limits, diagnostics, production refusals;
  - storage: admission, owner lock, control seed;
  - cursors;
  - activation;
  - the method log;
  - the spike verifier tests.
- **`tests/service.rs` (loopback):**
  - the spike's full identity-negative matrix (issuer, audience, tenant, client, owner,
    workspace, organisation, lifetime, algorithm, key headers, JWKS storms, unknown
    agent);
  - `service_info` shape and listing;
  - dispatch-order refusals;
  - envelope and tenant checks;
  - `history_status` absent then present;
  - `history_coverage` validation, frozen shape, paging, scope-bound and tampered
    cursors;
  - reload revocation and refused reloads;
  - maintenance and the health listener;
  - the stateless, bounded transport;
  - the org-wide assertion audience bound only by the upstream token;
  - public metadata;
  - concurrent rmcp clients of one user;
  - audit lines without secrets.
- **History store:** `coverage_groups` mapping, gaps, offline-gap and missing archive;
  `Cloud::from_parts` equivalence.

Limitations:

- Chunk reauthorization applies to `artifacts_download`, which arrives in M6. Every M2
  call is reauthorized per request.
- Secret files are validated for form only until M4 opens them.
- Live gateway proof of the production server is M8.

Verification (`.local/dev/runtime-mcp/m2/run2/`):

| Check | Result |
|---|---|
| fmt and Clippy | clean |
| `AIRS_REQUIRE_PARITY=1 cargo test --workspace` | 3901 passed, 0 failed, 5 ignored |
| `cargo +1.89.0 check --workspace --all-targets --locked` | passes |
| Prototype workspace | passes |
| CLI graph | no history store, server, rmcp, axum, jsonwebtoken or SQLite; `hyper-util`/`tokio-rustls` are reqwest's client side, as before M1 |

The workspace graph has one version each of axum 0.8.9, rmcp 3.8.0, jsonwebtoken
11.1.0, hmac 0.12.1, reqwest 0.13.5, scm-sdk 0.2.4, serde_json, tokio and rustls.
`preserve_order` is absent from both graphs. `hmac` 0.12 (already in the lock through
the test harness) is the one dependency the server adds beyond the M0 pins.


### M2 review 1 — 8.0/10, revise (preserved)

Reviewer: independent Codex (`.local/dev/runtime-mcp/m2/review-1/result.md`; manifest
`51b1b2e5…`, 654 files, unchanged before and after). Material findings:

1. Coverage aggregation dropped application scope (`scope_kind`, `selected_app_ids`)
   and unioned intervals across different sources in one category, so it could report
   a scoped or split traversal as tenant-wide `complete`.
2. Reload accepted `allowed_hosts`/`max_request_body_bytes` changes that the
   already-built transport never applied.
3. Activation re-read the source after validation and published before full
   validation (key sources, restart restrictions), so a refused reload could replace
   the private copy. Startup selected storage from an earlier read.
4. Production logs kept spike claim diagnostics (assertion `iss`/`aud`/workspace/
   organisation values). Audit lines lacked the time and approved scope digest.
5. rmcp transport refusals reached clients unnormalized (header text reflected in a
   400).
6. The cursor tests did not prove parameter, principal or epoch binding, or that a
   result-set change is `conflict`; there was no bounded-read timeout test.

Minor: the bundle omitted the synthetic signing fixtures (excluded `*.pem` private
keys).

The review confirmed:

- both tokens are required with no fallback;
- owner matching, tier intersection and the central side-door refusal;
- one `Live` snapshot per request;
- catalogue-derived tiers, schemas and limits;
- correct HMAC and seed handling;
- the bounded 2 s reader;
- `Cloud::from_parts` equivalence;
- tenant-free metadata and health;
- complete config validation;
- the package boundary.

### M2 round 2 corrections (submitted for review 2)

1. **Coverage** (`airs-history-store/src/inspect.rs::coverage_groups`, contract §4.3
   note).
   - Scope is kept: selection and application traversals count only for their named
     applications, and unknown scope is never complete.
   - Completeness is decided per underlying source; a category is complete only if
     every source in it is complete and gapless, and its gap is the largest source gap.
   - The offline-gap marker sets state without adding a gap.
   - Tests:
     - `coverage_is_decided_per_source_and_never_merged_into_completeness`, where two
       half-covering inventory sources give partial with a 2 h gap;
     - `a_scoped_traversal_never_becomes_tenant_wide_and_unknown_scope_is_never_complete`;
     - `an_offline_gap_marks_its_scope_falling_behind_without_inventing_a_gap`.
2. **Reload.** `allowed_hosts` and `max_request_body_bytes` are restart-only. The reload
   test refuses both, and the previous configuration stays active.
3. **Activation.** The source is read once as bounded bytes, and that exact candidate is
   fully validated (restart-only check, verifier key sources) before the same bytes are
   published, read back, compared and activated. Startup uses the same bytes for
   storage selection. `activation_publishes_and_loads_only_the_private_copy` shows that
   an invalid candidate and a valid one refused by the reload check both leave the
   previous private copy byte-for-byte.
4. **Logging.**
   - Spike claim diagnostics (shapes, assertion `iss`/`aud`/workspace/organisation
     values) are removed from production logs. `allowed` holds event, method,
     permission and revision; `denied` holds code, reason and the ungranted
     `(sub, client_id)`.
   - Audit lines add `time`, `scope_digest` and `job_id`.
   - `audit_lines_hold_only_identifiers_and_outcomes` checks every emitted log line
     against its event's allowlist, and that no token or argument value appears.
5. **Transport errors.** `normalize_transport` replaces every rmcp error status body
   with a fixed one per status, and every JSON-RPC error message with fixed text.
   `transport_errors_never_reflect_client_text` sends a sentinel in a protocol header,
   a method name, malformed JSON and an invalid parameter shape; it never comes back.
6. **Cursors and bounded reads.**
   - `coverage_cursors_bind_parameters_and_principal_and_detect_changes`: two valid
     ranges, an independently granted second principal, and a new import between pages
     that gives `conflict`.
   - `scope_digests_bind_epoch_tool_tenant_principal_and_parameters`: the real scope
     construction with another epoch, tool, tenant, principal or parameter set.
   - `a_read_past_its_budget_is_refused_and_cancelled`.
   - The old 2099-range case now uses a valid longer range.
7. **Minor.** The `tests/fixtures/*-private.pem` files are synthetic, disposable test
   keys (their README); the review prompt now says so explicitly.

**Present but outside M2's scope.** The tree also contains M3 work in progress, which
M3's own review will score:

- `control.rs`, `control.sql` and `jobs.rs`;
- the `jobs_*` and `collection_start` tools;
- control-store startup and recovery in `run.rs`.

M2 review 2 checks only that it introduces no M2 regression.

Verification for round 2 (`.local/dev/runtime-mcp/m2/run4/`):

| Check | Result |
|---|---|
| fmt and Clippy | clean |
| `AIRS_REQUIRE_PARITY=1 cargo test --workspace` | 3924 passed, 0 failed, 5 ignored |
| `cargo +1.89.0 check --workspace --all-targets --locked` | passes |
| Prototype workspace | passes |
| CLI graph gates | unchanged |

Run3 failed Clippy (three `let_and_return` lints in the M3 control store), was fixed and
rerun as run4.

### M2 review 2 — 8.8/10, revise (preserved)

Source: `.local/dev/runtime-mcp/m2/review-2/result.md`, verdict revise.

Five of the six review 1 findings were confirmed resolved:

- coverage scope and source aggregation;
- restart-only transport settings;
- production logging and the audit record;
- transport-error normalization;
- the cursor and timeout evidence.

Two material findings remained.

1. **A refused activation could still replace the private configuration.**
   - Activation built a validated `Live` and discarded it, published the candidate, then
     built a second `Live`. That second build reread the assertion public-key file.
   - The reviewer reproduced it with a FIFO serving a valid key on the first read and
     invalid bytes on the second. Activation failed, memory kept `example-1`, and the
     private copy held `example-2`.
2. **M3 control contention bypassed the two-second read budget.**
   - `history_coverage` and `jobs_list` took the epoch through a synchronous, unbounded
     control-store mutex on the async runtime. `service_info` also took the epoch
     synchronously.
   - The reviewer held the lock for 2.6 s; a coverage call under a 2 s outer timeout
     blocked for 2.585 s.

No minor findings were outstanding. The synthetic signing fixtures were present.

### M2 round 3 corrections (submitted for review 3)

1. **Activation.** `run::activate` reads the key sources only while validating, and returns
   that same prepared `Live`.
   - The candidate bytes are staged as a private 0600 file beside the active copy
     (`storage::stage_private`), read back and compared, then renamed over it atomically.
   - A refusal at any step drops the staged file, leaving the previous private copy and
     the in-memory configuration untouched.
   - Test `activation_reads_key_sources_once_and_refusals_after_validation_change_nothing`:
     - Key source: a FIFO serving a valid key once, then bytes that are not a key.
     - Activation succeeds with the validated revision, and the key source is opened only
       once.
     - A configuration directory that refuses writes after validation leaves the previous
       copy and no staged file.
   - The test fails when the old second `Live::new` is restored, so it detects the defect.
2. **Control contention.**
   - The active epoch and cursor key are published once, when storage is opened. The epoch
     changes only through offline maintenance under the exclusive owner lock, so
     `service_info`, `history_coverage`, `jobs_list` and `collection_start` read it without
     the control store.
   - Every request-side control transaction goes through `Storage::control_bounded`. It
     runs off the runtime (`spawn_blocking`), counts lock queueing within the 2 s budget,
     and refuses `storage_unavailable` without running the transaction once the budget
     passes.
   - Covered tools: `jobs_get`, `jobs_cancel`, `jobs_list` and `collection_start`. The
     submission and its read-back are one bounded call.
   - The worker no longer runs on the async runtime; it has its own OS thread
     (`airs-mcp-worker`).
   - Test `control_contention_never_blocks_the_runtime_or_escapes_the_budget`, through the
     handlers on a single-threaded runtime with the store held past twice the budget:
     - coverage is not delayed (under 500 ms);
     - `jobs_list` and `collection_start` are refused at the budget (within 300 ms);
     - a concurrent ticker keeps advancing (at least 50 ticks);
     - the refused submission was never applied.
   - The test fails when the epoch is read through the control store again.

### M2 review 3 — 8.9/10, revise (preserved)

Source: `.local/dev/runtime-mcp/m2/review-3/result.md`.

Both review-2 findings were confirmed fixed:

- Key-source FIFO-to-file rotation probes passed through both activation branches.
- Contention probes over HTTP for `service_info`, `history_status` and
  `history_coverage` each completed within 500 ms.

The review-1 resolutions held. One new material finding:

1. **A failure after the rename left disk and memory split.**
   - `Staged::commit` renamed the file, then opened and synced the directory, both
     fallibly. `activate` turned that failure into a refusal, so SIGHUP kept the old
     `Live` while the new file was active on disk.
   - The reviewer reproduced it by making the directory mode 0300 after staging.
   - The round-3 test removed write permission before staging, so it did not cover this
     case.

No minor findings.

### M2 round 4 corrections (submitted for review 4)

1. **`stage_private`** opens the directory before writing the candidate, so publication
   needs no further fallible open.
2. **`Staged::commit`** returns `Err` only when the rename fails, in which case nothing
   was published. Otherwise it returns `Published { durable }`.
3. **`activate`** installs the prepared configuration whenever the rename happened. A
   failed directory sync is logged as `config_publish_not_durable`, never a refusal.
   The contract §11 activation note states this.
4. **Test `failures_after_publication_never_split_disk_and_memory`:**
   - the reviewer's 0300-after-staging probe now publishes durably;
   - a fault-hooked directory-sync failure after the rename still activates the
     prepared configuration, with the revision, the returned `Live` and the disk copy in
     agreement;
   - turning a non-durable publication back into a refusal makes it fail.

   The round-3 test's comment now says it covers refusal before publication.

### M2 review 4 — 9.0/10, revise (preserved)

Source: `.local/dev/runtime-mcp/m2/review-4/result.md`.

- The review-3 finding was confirmed fixed: the 0300-after-staging and injected
  sync-failure probes passed through both activation branches.
- The reviewer found no other material M2 regression from the M3 changes.

One material finding:

1. **A log failure after publication could still split disk and memory.**
   - `activate` logged the non-durable warning before returning, using `println!`, which
     panics when stdout fails.
   - The reviewer combined an injected sync failure with stdout on `/dev/full`. The
     SIGHUP task panicked before the swap, leaving `example-2` on disk and `example-1` in
     memory.

### M2 round 5 corrections (submitted for review 5)

1. **`run::log` never panics.** It writes with `writeln!` and ignores the error, so a
   failed or closed stdout drops the line.
2. **`activate` emits nothing.** It returns `Activated { live, durable }`.
3. **The SIGHUP loop calls a new `reload_from`.** It activates, swaps (installs), and only
   then logs `config_publish_not_durable` and `config_reloaded`. Startup logs a
   non-durable publication only after activation as well.
4. **Test `reloads_install_what_they_publish_even_when_sync_and_logging_fail`:**
   - Faults: an injected directory-sync failure, plus a test hook making every log write
     fail as `/dev/full` does.
   - Two consecutive reloads through `reload_from` each install what they published:
     memory, the returned revision and disk agree, and handling continues.
   - Making the failing write panic as `println!` would fails the test.

Post-freeze test fix (no product change):
`run::tests::a_grant_revoked_during_a_job_blocks_the_next_claim` assumed claim order
between two jobs submitted in the same millisecond, which a random job ID breaks. It now
asserts that exactly one job ran and succeeded and that the claim after the reload was
blocked.

### M2 review 5 — 9.1/10, ACCEPT

Source: `.local/dev/runtime-mcp/m2/review-5/result.md`, verdict accept, with no material
findings.

The reviewer verified the review-4 fix:

- an injected sync failure plus real stdout on `/dev/full`, through startup-style
  activation and reload;
- real SIGHUP signals through a copy of the production handling loop (success, refusal,
  success), with handling continuing and published and installed revisions agreeing.

The reviewer found no M2 regression from the M3 changes. Fifteen in-process HTTP tests
covered the M2 exit gate.

Minor findings, fixed after acceptance:

1. **`main.rs` printed with `println!` and `eprintln!`,** which panic on a failed stream.
   - Fix: checked `out`/`err` writers keep the intended exit status.
   - Test: `commands_keep_their_exit_status_when_output_fails`. `check-config` exits 0
     with stdout on `/dev/full` and 1 with stderr on `/dev/full`.
2. **`reload_from` cloned the revision string before installation.**
   - Fix: it now installs with an `Arc` clone (no allocation) and derives diagnostics
     afterwards.

Both fixes are in the tree that M3 review 4 inspects for M2 regressions.

## M3 — Durable control state and recovery

Status: **ACCEPTED at 9.2/10 by independent review 5** (2026-11-06; reviews 1–4 scored
6.4, 8.0, 8.7 and 9.0). The acceptance covers M3 and its inspected M2 integration, not M4
execution or M5 lifecycle tools. Each M3 review also qualifies the combined M2+M3
tree, which the plan requires before M4. M2 was scored separately and is accepted.

### M3 implementation

- **M3.1 Job, idempotency, cancel and state schema.**
  - `control.sql` implements the §9.1 tables and `jobs.rs` the frozen §10 edges
    (19 pairs; terminal states admit none).
  - `control.rs` makes every change one transaction:
    - C1 submission, with the same-key/different-digest/other-epoch rules and joining of
      an equivalent live job;
    - queue bounds giving `overloaded`;
    - explicit, idempotent cancellation that completes only at a checkpoint or an unheld
      scheduler pass;
    - checkpoints that carry the archive `operation_id`.
  - Tools: `jobs_list`, `jobs_get` and `jobs_cancel` show only the caller's own jobs,
    and `collection_start` returns the durable job.
- **M3.2 Reconciliation and singleton worker.**
  - Recovery runs under the exclusive owner lock before any request: open reservations
    become `uncertain` (consumed). `running` jobs keep their state with the claim
    released, so the next claim resumes them from their checkpoint (round 2; there is
    no §10 recovery edge). Dead-worker cancellations are released.
  - The single worker claims fairly (oldest last claim first), only executable
    operations, and rechecks grants at claim.
  - C4's archive lookup by `operation_id` is the executor's resume duty: the M4
    collection executor implements it, and M3 exposes the resume point.
- **M3.3 Startup recovery, backup and restore.**
  - C12: a foreign, unsupported or damaged store is refused and never rewritten.
  - C9: a `VACUUM INTO` backup set whose manifest is published last.
  - C10/C11: a validated, no-clobber restore into a pending epoch, with restored jobs
    blocked, reservations consumed and cooldowns held at least 24 hours, followed by
    explicit activation.
  - Offline `control-backup`, `control-restore` and `control-activate-epoch`
    subcommands.
- **M3.4 Fair dispatch, policy checks, durable allowances.**
  - Reports allowances are charged durably by Chicago admission day under a
    high-water guard (no reset or reopening on rollback or restart).
  - The scheduler pass handles retries and deadlines.
  - Uncertain sends are never refunded.

Tests:

- **`jobs.rs` (3):** edge table, cancellation, names.
- **`control_tests.rs` (17):**
  - C1, including a real process death mid-transaction that leaves a hot journal,
    rolled back;
  - queue bounds;
  - C2, including a real process death after a reservation;
  - resume from checkpoint, cancellation, fair claims with grant recheck and deadline,
    foreign or damaged stores, allowances with rollback;
  - joining;
  - C9 backup completeness and tampering, and C11 restore, pending epoch, inactive jobs,
    consumed sends, cooldowns, activation and old keys.
- **`worker.rs` (5):** run with reserved sends and result; cancellation at a checkpoint
  stops sends; retry, requeue and deadline with no refund; unexecutable operations stay
  queued and revoked grants block; worker death mid-run resumes from its checkpoint with
  its send consumed.
- **`tests/service.rs`:** jobs through the MCP endpoint (idempotent submission, conflict,
  stale epoch, joining, validation, ownership, paging, cancellation, frozen shapes).
- **`tests/maintenance.rs`:** the offline backup, restore and activation commands
  through the binary.

Limitations: no executor exists until M4, so accepted collections stay queued. The
lifecycle tools that expose backup, restore and activation over MCP are M5.

### M3 review 1 — 6.4/10, revise (preserved)

Source: `.local/dev/runtime-mcp/m3/review-1/result.md`. The two known M2 review-2
findings were excluded from the score. The reviewer reproduced 12 defects with
scratch-only probes.

Material findings:

1. **Database loss reused the old epoch.** A missing database with its seed retained was
   treated as first initialization.
2. **Restore reset spent allowances and skipped recovery restrictions.**
   - Missing cooldowns stayed absent.
   - Older allowance counters replaced newer consumption.
   - Schedules stayed enabled.
3. **Refused databases could be rewritten before validation.**
   - Opening switched a foreign database to WAL.
   - Hot-journal recovery ran on the original, not on a verified copy (C12).
4. **Restore trusted the manifest's identity.** The staged copy's internal schema, tenant
   and epoch were not validated.
5. **Recovery and restore bypassed the frozen state machine.** They wrote
   `running → queued` and transitions to `blocked_policy` outside `transition()`.
6. **Joining an equivalent job ignored ownership and doomed states.** A submission could
   join another principal's job, or one already `cancel_requested`.
7. **Job handlers blocked the async runtime with no deadline.** This covered `jobs_get`,
   `jobs_cancel`, the submission and the scheduler.
8. **The worker could outlive exclusive ownership.** It was detached, never joined, and
   started before startup completed.
9. **Claim-time grant checks could use a stale configuration snapshot.**
10. **`jobs_list` pagination had no snapshot binding or adaptive page size.**
    - A constant snapshot meant an inserted job repeated an item instead of returning
      `conflict`.
    - A 30-job default page exceeded the result bound.
11. **The durable authority transitions were missing.** Contract §3 requires preview,
    approval and consumption as control-store transactions.

Minor findings:

- `retry_after_seconds` was lost through dispatch.
- A deadline failure recorded no failure result or gap.
- The C1 crash test inserted a job by hand instead of interrupting `submit()`.

### M3 round 2 corrections (submitted for review 2)

Each finding has a regression test. The tests marked † were mutation-checked: each
fails when its fix is reverted.

1. **Loss (C11).**
   - The control directory is marked `provisioned` once a store exists, so a missing
     database there is a loss, not a first start.
   - A lost store is rebuilt without reusing the seed's epoch. The new store gets a fresh
     epoch, pending activation, in quarantine. Old keys stay `control_restored`, and the
     day's allowance is spent.
   - Test: `a_lost_database_never_reuses_the_seed_epoch` †.
2. **Quarantine.**
   - Restore and loss both enter a persisted recovery quarantine, and activation does not
     end it. While quarantined, the worker claims nothing.
   - The current admission day's allowance is exhausted, with the high-water mark at now.
   - Every plane holds a cooldown of at least 24 h, including planes absent from the
     backup.
   - Schedules are disabled with an effective time; their rows are kept.
   - The quarantine ends only through the explicit `control-reconcile` maintenance
     command.
   - Tests: `a_restore_never_resets_spent_allowances_or_reopens_schedules` † reproduces
     the reviewer's probe (back up, spend, restore, activate, and spending is still
     refused); the c11 test; the binary maintenance test.
3. **C12, validate before touching.**
   - `Control::open` copies the database and its sidecars into a private directory,
     opens only the copy read-write (hot-journal rollback and WAL replay happen there),
     and validates it. A refused original is never touched.
   - When the original had sidecars, it is preserved under `preserved-<ms>-<uuid>/` and
     the verified copy is published in its place.
   - Tests: `a_refused_database_and_its_sidecars_are_never_modified` (a foreign WAL
     database keeps every byte); `a_recovered_store_preserves_the_original_and_publishes_a_verified_copy`.
4. **Restore identity.**
   - Before the staged copy is changed or published, it is validated against the
     manifest and this service: integrity, schema, tenant, required tables,
     `foreign_key_check` and the manifest's epoch as active.
   - Test: `a_restore_validates_the_copy_itself_not_only_its_manifest` (a forged-manifest
     foreign tenant, and an unsupported internal schema with a recomputed manifest; both
     place nothing).
5. **State machine.**
   - `record` now takes a `Trigger` and validates the edge with `transition()`. Every
     state write goes through it.
   - Recovery no longer writes `running → queued`: it releases the claim and keeps the
     job `running`, and the next claim resumes it.
   - Restore keeps job states, and the quarantine gates them.
   - `assert_events_are_contract_edges` checks every recorded event in the recovery,
     restore, deadline and crash tests.
6. **Joining.**
   - A submission joins only the same submitter's job, and only while that job is
     `queued`, `running` or `waiting_retry`.
   - Test: `equivalent_live_collections_join_only_the_submitters_executable_job` † (no
     cross-principal join; no join of a cancelling or terminal job).
7. **Blocking handlers.**
   - All request-side control work goes through `Storage::control_bounded` (M2 round 3):
     off the runtime, with queueing inside the budget. Reads get 2 s; submission and
     cancellation get 10 s (`SUBMIT_BUDGET`, contract §6).
   - The scheduler and claims run on the worker's own thread.
   - Test: `control_contention_never_blocks_the_runtime_or_escapes_the_budget`.
8. **Worker lifetime.**
   - The worker starts only after every fallible startup step. `WorkerHandle` stops and
     joins it, also from `Drop`, so an unwinding exit is covered too. `serve` stops it
     before releasing ownership.
   - Test: `stopping_the_worker_joins_it_before_ownership_is_released`. A step in
     progress finishes before `stop` returns, and nothing is claimed afterwards.
9. **Claim-time configuration.**
   - The worker re-reads the configuration before every claim.
   - Test: `a_grant_revoked_during_a_job_blocks_the_next_claim`. A reload inside the
     first job's executor revokes the grant, and the second job becomes `blocked_policy`.
10. **`jobs_list`.**
    - Each page binds `list_digest`, a digest of the caller's whole filtered set read
      under the same lock, so a changed set is `conflict`.
    - A page shrinks until its complete result fits the agent bound.
    - Test: `jobs_pages_fit_their_bound_and_bind_the_result_set`. 40 jobs come back over
      several bounded pages with no repeats, and an inserted job is `conflict`.
11. **Durable authority transitions** (`authority_store.rs`):
    - Every §3 model transition is one control-store transaction bound to the active
      epoch: preview, `execute_direct`, `confirm`, `execute_with_approval`,
      `upload_begin`/`chunk`/`finalize`/`abort` and `preview_import`.
    - Exact retry bindings: preview, digest, generation and approval.
    - The previews and approvals tables were extended, and `upload_sessions` and
      `authority_keys` added.
    - Nine tests mirror the model's cases against the store. They include a test-only
      failpoint aborting each transition after all its writes and before commit, which
      leaves no job, event, key binding, consumed flag or upload change.
    - The MCP tools stay M5.

Minor findings:

- **`retry_after_seconds`** now reaches the client through dispatch (the `Failure` type).
  Test: `overloaded_refusals_keep_their_retry_hint`.
- **Deadline failures** record a fixed `result_failure` and a gap (new `jobs.gaps_json`)
  atomically, from claim and from the scheduler. Test:
  `deadline_failures_record_their_result_and_gap`.
- **C1 crash test.** Test-only crash points inside the real `submit()` transaction, on
  both the new-job and the join path, end the child process before commit. Test:
  `c1_dying_inside_submit_leaves_nothing_on_both_paths` (nothing committed; recovery
  through C12; a retry succeeds once).

Schema notes: `jobs.gaps_json` was added and `meta` admits a `quarantine` key. Schema 1
has not been deployed, so these are definitional, not migrations.

### M3 review 2 — 8.0/10, revise (preserved)

Source: `.local/dev/runtime-mcp/m3/review-2/result.md`.

The reviewer confirmed eight round-1 findings fixed: loss, refusal preservation, state
edges, joining, blocking handlers, worker lifetime, claim-time configuration and
pagination. It confirmed three partly fixed. It judged as reasonable the
orphaned-running design, the provisioned marker, quarantined restored states and
copy-then-verify.

Six material findings:

1. **Reconciliation bypassed activation.** Claims checked only the quarantine.
2. **Unknown per-job sends were lost on restore.** In addition, `reserve_send` ignored
   recovery cooldowns.
3. **Authority-created jobs bypassed the queue bounds.** The reviewer queued 65 for one
   principal.
4. **Authority jobs lacked durable executable parameters.** Only the digest was stored.
5. **Restore validation checked table names, not structure.** A missing column passed.
6. **Policy-blocked jobs never reconsidered a restored grant.**

Minor findings:

- Stored gaps were not on the wire.
- The join crash test did not show a hot journal.
- The revocation test raced on submission order. This was already fixed after the
  freeze, as recorded under M2 round 5.

### M3 round 3 corrections (submitted for review 3)

Each finding has a regression test, mutation-checked (†): the test fails when the fix is
reverted.

1. **Activation and reconciliation.** A claim now requires an active epoch as well as no
   quarantine, and `reconsider_blocked` applies the same gate. Test:
   `claims_need_both_activation_and_reconciliation_in_either_order` †.
2. **Unknown sends and cooldowns.**
   - Entering quarantine charges every non-terminal job `uncertain` reservations: one per
     plane plus the two-send per-job Reports allowance.
   - `reserve_send` refuses during any cooldown on the plane, with
     `upstream_unavailable` and the wait.
   - Test: `restored_jobs_keep_unknown_sends_and_respect_recovery_cooldowns` †. It runs
     the reviewer's probe: back up a running job, record two sends, restore, activate and
     reconcile. The sends stay charged, and a reservation is refused for the 24 h hold.
3. **Authority queue bounds.**
   - `Control::admit_queue` is now shared by `submit` and the authority `admit_job`.
   - An overload rolls the whole transition back, and the preview stays open.
   - Test: `authority_jobs_respect_the_queue_bounds_and_leave_previews_unconsumed`. The
     retry hint is kept, and a retry with the same key succeeds once there is room.
4. **Durable authority parameters.**
   - `preview` and `preview_import` take the action parameters and store them canonically
     (new `previews.params_json`); the store computes the digest from them.
   - Each job carries `action_params`, and `admit_job` checks that they still match the
     digest.
   - `upload_begin` requires the declared `total_bytes`.
   - Tests: `executed_jobs_carry_the_previewed_parameters_across_reopen` and
     `uploads_are_bound_to_their_declared_size`.
5. **Structural validation.**
   - `validate` compares the copy's `sqlite_master` (type, name, sql) with a store freshly
     created from the schema.
   - Test: `stores_with_the_right_tables_but_wrong_structure_are_refused` † runs the
     reviewer's probe. Dropping `jobs.gaps_json` is refused on restore (nothing placed)
     and on open (bytes untouched).
6. **Blocked jobs.**
   - `Control::reconsider_blocked` runs every worker tick under the current grants and
     uses the validated `PolicyCleared` edge, keeping deadlines. It writes no event for a
     job still blocked.
   - Test: `a_restored_grant_requeues_a_policy_blocked_job` †.

Minor findings:

- `job_view` serializes `job.gaps`. Test: `recorded_gaps_reach_the_job_view`.
- The C1 test now asserts the hot journal on the new-job path. Its claim for the join
  path is narrowed to "nothing of it survives".
- The revocation test is order-independent.

Schema notes (still undeployed schema 1): `previews.params_json` was added.

### M3 review 3 — 8.7/10, revise (preserved)

Source: `.local/dev/runtime-mcp/m3/review-3/result.md`.

The reviewer confirmed these review-2 findings fixed:

- the reconcile/activation gate;
- authority queue bounds;
- structural validation;
- blocked-job reconsideration;
- all three minors.

Two material findings remained:

1. **Unknown consumption was still bounded at five.**
   - A running collection that sent 200 times after the backup counted as five after
     restore.
   - With a clock rollback, the recovery hold expired before the job's deadline, after
     which the job could be claimed and could reserve again.
2. **An upload declaration was optional.** An `upload_admit` preview of `{}` let a proof
   begin a 1024-byte upload.

Minor findings:

- The C1 test checked that the journal existed, not that it had a hot header.
- The M3 summary still had obsolete status and recovery wording.

### M3 round 4 corrections (submitted for review 4)

1. **Exhausted budgets.**
   - New `exhausted_jobs` table. Entering quarantine (restore or loss) marks every
     non-terminal job, replacing the fixed synthetic reservations.
   - `reserve_send` refuses a marked job with `quota_exhausted`, at any time.
   - Its next claim fails it with the gap "recovery left this job's consumption unknown;
     its budget is spent" and a `quota_exhausted` `result_failure`.
   - Nothing removes the mark: not reconciliation, cooldown expiry or a clock change.
   - Test: `restored_jobs_never_regain_their_budget` † runs the reviewer's probe: 200
     post-backup sends, then restore, activate and reconcile. Reservations are refused
     during the hold, after it and days later, and the claim fails the job.
   - The c11 and order tests now show that only work under the new epoch runs.
2. **Upload declarations.**
   - `preview` refuses an `upload_admit` without a complete, valid declaration, per the
     frozen `lifecycle_preview.params.upload` (kind, `total_bytes` 1..16 GiB, `sha256`,
     `expect_tsg`, `expect_environment`, and no other field).
   - `upload_begin` requires the declared length and refuses a stored preview without a
     declaration (defence in depth), leaving it unconsumed.
   - `upload_finalize` now takes the finished bytes' SHA-256 and compares it with the
     declared one, instead of trusting a caller's boolean.
   - Test: `uploads_need_a_complete_declaration_and_finish_with_its_hash` † covers six
     malformed declarations, a wrong length with the preview left open, a wrong hash, and
     a stored preview whose declaration is gone.

Minor findings:

- The C1 test asserts SQLite's hot-journal header magic.
- The M3 summary is updated: current status, and released claims with `running`
  preserved.

### M3 review 4 — 9.0/10, revise (preserved)

Source: `.local/dev/runtime-mcp/m3/review-4/result.md`.

The reviewer confirmed both review-3 findings and both minors fixed. Its probes covered:

- 200 post-backup sends, with rollback and hold expiry before the deadline;
- exhaustion surviving a reopen and a second backup and restore;
- the full set of malformed upload declarations.

One material finding:

1. **A timed-out submission could run later from Tokio's blocking queue.**
   - `control_bounded` checked the deadline only while waiting for the lock.
   - A queued task that started after expiry and got the lock at once ran its
     transaction.
   - The reviewer saturated a one-thread blocking pool: `collection_start` returned
     `storage_unavailable` after 10 s, then created the job once the pool was released.

Minor findings:

- A recovery-exhaustion failure used the deadline's reason text.
- The `VACUUM INTO` backup copy was 0644 under umask 022 (inside a 0700 directory).

### M3 round 5 corrections (submitted for review 5)

1. **Late execution.**
   - `control_bounded` checks the absolute deadline again after taking the lock and
     immediately before the transaction, so a task that starts late never runs it.
   - On timeout it also aborts the pending blocking task, which drops it if it has not
     started; a transaction already running finishes, idempotent as before.
   - Test `a_task_started_after_its_budget_never_runs`: a one-thread blocking pool is
     saturated past a 200 ms budget, the call is refused, and nothing runs after the pool
     frees. The test fails only when both guards are removed; each alone suffices.
2. **Minor findings.**
   - A failure's reason is now its gap text, naming the cause: a deadline or recovery
     exhaustion. Asserted in `restored_jobs_never_regain_their_budget`.
   - `backup_into` sets the copy to 0600 before reading it back and before publishing the
     manifest. Asserted in the C9 test.

### M3 review 5 — 9.2/10, ACCEPT

Source: `.local/dev/runtime-mcp/m3/review-5/result.md`, verdict accept, with no material
or minor findings outstanding.

The reviewer verified the round-5 corrections:

- **Late execution:** a saturated single-thread blocking pool was probed through
  `collection_start`, the job and schedule handlers, and cancellation after its
  ownership read; no timed-out call mutated anything late. With both guards removed the
  defect reproduced.
- **Failure reason:** recovery exhaustion names its own cause, with the deadline still in
  the future.
- **Backup permissions:** the copy is 0600 under umask 022.

The M3 exit gate was re-checked end to end, and no M3-caused M2 regression was found.

## M4 — Collection, credentials and schedules

Status: **ACCEPTED at 9.1/10 by independent review 14** (2026-11-06; reviews 1–13 scored
5.8, 7.2, 8.0, 8.4, 8.6, 8.7, 8.8, 8.8, 9.0, 9.0, 9.0, 9.0 and 9.0).

### M4 implementation (submitted for review 1)

- **M4.1 Collector in service context.**
  - `airs_tenant.rs` builds the AIRS tenant from the secret files.
  - `collection.rs` is the executor, with configured tick ceilings and zero Reports
    calls.
  - `SyncRequest.run_id` lets the service mint the archive run ID before the run (C4).
  - `run_summary.rs` (history store) reads one run's counts, and `interrupt_run`
    closes a run left `running`.
  - The frozen `result_collection` is produced.
- **M4.2 OAuth and credentials.**
  - `ReadBudget` counts admitted `oauth.token` requests, which are recorded with the
    run's progress.
  - Denials are recorded in `credential_denials` and never retried.
    `credentials_restored` runs on a SIGHUP that changes the management revision.
  - A management 429 becomes a 15-minute control-store hold, checked before any
    reservation.
- **M4.3 Schedules.**
  - `schedule_store.rs`: slot ticks, coalesced catch-up and slot keys, with the
    quarantine respected.
  - The `schedules_list`, `schedules_set` and `schedules_disable` tools, with atomic
    keyed results (a savepoint around the change and its record).
- **M4.4 Enrichment.** The `Enrichment` executor, with the Reports allowance charged
  before any send, and the frozen `result_enrichment`. The `enrichment_start` report
  lookup is deferred to M6 (contract §6.1).
- **M4.5 Receipts.**
  - `receipts_ingest` with association-only attribution and the bounds above.
  - `repository::ingest_client_receipts`: the conservative CLI receipt path, under its
    own `mcp/` origin namespace, returning accepted and duplicate counts.
  - Idempotency by batch key (`ingest_results`).

Tests: `crates/airs-mcp/tests/collection.rs`, against the loopback AIRS fixture.

1. A collection job records the frozen result, zero Reports calls, OAuth sends equal to
   the fixture's token requests, and its run ID checkpointed first.
2. C4 resume of a finished run makes no request.
3. C4 resume of a run with no archive row collects under the checkpointed ID.
4. C4 resume of a run left `running`: the run is `interrupted`, the job `partial`, and
   no request is made.
5. A 401 denial: at most one token request plus its replay. A denied revision then
   sends nothing, and rotation resumes both parked jobs with their deadlines kept.
6. Enrichment: one Reports transport for five selected examples, the frozen result,
   and the next job that day `blocked_policy` with no request.
7. A management 429: the run is `partial`, and the next job is `waiting_retry` with no
   request.
8. Receipts reconcile in either order (partial, then promoted by collection; or
   promoted at once), and a repeated key is a duplicate.

Unit tests:

- `schedule_store`: interval bounds, one job per slot, the slot key rejoining after a
  crash, coalesced catch-up (four slots give one job, and 96 more ticks no extra job),
  and quarantine.
- `tools`: receipts (wrong cloud, unassociated scanner, idempotent replay, key conflict,
  duplicates, the 200-record bound, non-attribution of a foreign application ID), and
  schedule tools (stable ID and revisions, replay and conflict, disable, daily kinds
  refused).
- `catalogue`: the implemented set and its idempotency flags.

Schema notes (still undeployed schema 1): the new control table `ingest_results`. In
the archive, `collection_runs` progress gains `token_attempts`, `selected_report_ids` and
`cached_examples`.

### M4 review 1 — 5.8/10, revise (preserved)

Source: `.local/dev/runtime-mcp/m4/review-1/result.md`.

Material findings:

1. Promoted `client_asserted` receipt facts changed provider verdict/action
   projections.
2. No fresh-token replay happened: SDK retries were 0, so one 401 produced one send.
3. Token and API outcomes were conflated:
   - OAuth successes masked denied API reads;
   - transient token failures were terminal;
   - a job resumed after rotation finished from its failed run.
4. Completeness was lost:
   - `strict: false` let missing data succeed;
   - recovery replaced the original bounds and applications and reported zero
     unfinished sources;
   - enrichment resumes reported success.
5. Every scheduled tick traversed a fresh 24 h.
6. Cancellation never reached the collector's session.
7. `receipts_ingest` recorded its key after its effects.
8. The Reports allowance was charged to the job-start day, once, with no per-transport
   reservation.
9. C2 settled `NotSent` when the summary lookup failed, and management counts included
   OAuth.
10. Schedules were overwritten instead of effective-dated, a future disable was listed
    as in effect, and completion was rolled back by early returns.
11. Receipt normalization had no service secrets.

Minor findings:

- A FIFO secret file could block the open.
- A refusal-only receipt batch opened or created the archive.

### M4 round 2 corrections (submitted for review 2)

Tests marked † were mutation-checked: each fails when its fix is reverted.

1. **Client claims.** `rebuild` skips `client_asserted` facts. The facts are kept, but
   they never enter the projection. Test `client_claims_never_change_the_provider_projection`
   † covers both arrival orders.
2. **Replay.** `ReadBudget::enable_auth_replay` makes the service's single explicit
   replay of an API 401: SDK retries stay 0, a 403 or a refused token request is never
   replayed, and the replay is admitted and journaled. The CLI leaves it off, so its
   traffic is unchanged. Test `a_401_gets_exactly_one_fresh_token_replay_and_a_403_none` †
   checks one 401 (read twice, two token requests, no denial), two 401s (no third read)
   and a 403 (one read).
3. **Classification.** `run_summary` classifies from the journal (API rows versus
   `oauth.token` rows):
   - denial: API reads all refused, or the token endpoint refusing the credentials,
     with no API data;
   - transient token failure: `waiting_retry`;
   - a resume after a credential-denied run starts a new run.

   Test `token_outcomes_are_classified_and_a_rotation_resumes_collecting` †: a 503 token
   endpoint gives `waiting_retry` with no denial; refused credentials park the job; after
   rotation the job collects again under a new run ID.
4. **Completeness.** Collection runs are strict. A job succeeds only when its durable
   record is complete (state, the progress field `incomplete_required_sources`, no
   unconfirmed reads). Resume keeps the original bounds and applications and reports
   at least one unfinished source when the record is incomplete. Enrichment resumes
   succeed only for a complete run.
   - Tests: the existing C4 resume tests, and the enrichment test, which now reports the
     fixture's honest `partial`.
   - Not mutation-run separately.
5. **Selector.** `inspect::polling_lower_bound` takes the oldest polled traversal minus
   15 min, within [now − 24 h, now − 59 min], and never before bootstrap. Test
   `a_later_tick_reads_only_since_the_last_traversal` † checks that the first tick reads
   `time_interval=24` and the next `1`.
6. **Cancellation.** A scoped watcher thread cancels the collector's `scm::Session` when
   the job becomes `cancel_requested`. Test `cancellation_stops_further_sends` †: a
   1.5 s delayed overview, cancelled at 0.4 s, ends `canceled`, with no session read
   and no request afterwards.
7. **Receipt keys.**
   - `ingest_reserve` binds the (caller, epoch, key) digest before any archive write;
     `ingest_record` replaces only a pending reservation.
   - Counts come from `run_summary::receipt_run_counts` on the deterministic per-key run.
   - Tests: `a_batch_key_is_bound_before_any_archive_effect` † (an archive-write failure
     still binds the key, and other records conflict); `receipt_batches_reserve_first_replay_stably_and_never_retain_service_secrets`
     (a replay after a simulated crash returns the identical result, and a pending key
     with other records conflicts with no write).
8. **Reports allowance.**
   - The `ReadBudget` Reports gate, installed by enrichment, charges `(1, ids)` to the
     admission day at each transport's admission, plus a durable per-transport
     `reports` reservation (`uncertain`, never released).
   - `reports_spent` blocks a spent day before any request.
   - Test: `enrichment_is_sparse_and_charged_to_the_daily_allowance` † asserts
     `(1, 5)` charged per transport, a run reservation plus the transport's own, and the
     next job `blocked_policy` with no request once the day is spent.
9. **C2.** The run reservation is settled `uncertain` whenever the summary cannot be
   read, never `not_sent`. Management and OAuth counts come from journal rows; the first
   collection test checks that OAuth equals the fixture's token requests.
10. **Schedules.** Versions are effective-dated (`versions` in the spec); the tick uses
    the version at its slot, and the list shows the one in effect now. The completion
    update commits before the decision transaction. Test
    `schedule_changes_take_effect_only_at_their_time` † (dated-version mutation): a
    future disable leaves the next slot running, the list changes at the effective time,
    completion survives a disabled tick, and a re-enable takes effect at its time. The
    tools test now dates its disable in 2099.
11. **Service secrets.** `receipts_ingest` registers the service's secrets (through
    `airs_tenant`) before normalization. Covered by
    `receipt_batches_reserve_first_replay_stably_and_never_retain_service_secrets` †: a
    `session_id` equal to a synthetic configured secret never reaches the archive bytes.

Minor findings:

- **FIFO secret files.** `read_secret` opens `O_NONBLOCK`, then checks the target with
  fstat. Test `a_fifo_secret_file_is_refused_without_blocking` (the old code would hang,
  so no mutation run).
- **Refusal-only batches** never touch the archive. Covered by the receipt test above †.

Also changed: `ReadBudget::run` takes `Fn` closures, so a read can be replayed; collect
call sites borrow their query.

### M4 review 2 — 7.2/10, revise (preserved)

Source: `.local/dev/runtime-mcp/m4/review-2/result.md`.

The reviewer confirmed fixed: receipt projection isolation, the single 401 replay,
per-admission-day Reports charging, reservation before receipt effects, effective-dated
schedules and completion, and both review-1 minors.

Material findings:

1. Polling progress came from coverage aggregation's earliest observation and counted
   partial segments: twelve quarter-hour traversals gave a four-hour selector, and a
   partial-only traversal advanced progress.
2. A successful peer read masked a permanent denial (`returned == 0` was required).
3. Retry and enrichment recovery finished unfinished work: a due transient retry
   finished from the failed run; enrichment after rotation finished the denied run; the
   enrichment no-archive-row case dropped its checkpointed run ID.
4. A credential rotation's new run got a fresh full budget: nothing bounded the
   logical job's cumulative traffic.
5. Recent-job recovery recomputed its bounds at recovery time.
6. Cancellation was polled every 100 ms; a request completing between polls could admit
   further sends.
7. Receipt secret protection failed open when a credential file could not be loaded.
8. `history_status` still returned the pre-collection placeholder values.

Minor finding: slots missed during downtime left no skipped-slot history.

### M4 round 3 corrections (submitted for review 3)

Each finding has a regression test that fails when its fix is reverted (all
mutation-checked).

1. **Progress.** `inspect::polling_lower_bound` reads, per windowed source and
   application, the latest supported-scope `traversed_as_of` segment and starts at the
   oldest of those; partial segments never advance, and no complete traversal means no
   progress. Only the `sessions` listing is windowed (`WINDOWED_SOURCES`).
   Test `polling_progress_is_the_latest_complete_traversal_and_partials_never_advance`
   (twelve quarter-hour traversals: at most a one-hour selector; partial-only: `None`).
2. **Terminal denials.** `RunSummary.terminal_denied` counts per-read terminal refusals
   from the journal (a 403, or a 401 not followed by a successful read of the same
   operation); any terminal denial is a credential denial whatever else returned.
   Test `a_peer_success_never_masks_a_terminal_denial` (session detail 403 with a
   successful overview: `needs_credentials`, denial recorded, the next job sends
   nothing).
3. **Continuation.** A job that waits is parked with a `continue` checkpoint (reason,
   scope, runs); its next claim makes a new run for the unfinished work. Enrichment
   shares the recovery rules and keeps a checkpointed run ID without an archive row.
   Tests `a_transient_token_failure_retries_the_unfinished_work_when_due` and
   `an_enrichment_run_without_an_archive_row_keeps_its_checkpointed_id`. The
   enrichment-after-rotation path uses the same shared rule as collection
   (`credential_denied` → new run) and is covered by the collection rotation test, not
   a dedicated enrichment test.
4. **Cumulative allowance.** The checkpoint carries every archive run of the logical
   job; their durable consumption (API + OAuth + Results + unconfirmed; an unreadable
   run counts as the whole allowance) is subtracted and only the remainder is passed to
   the run's `ReadBudget`. Enrichment's Reports transports are capped at two per job
   across runs (`Control::plane_sends`). Test
   `continuation_runs_get_only_the_jobs_remaining_allowance` (two sends remain: the run
   stays within them; none remain: `quota_exhausted`, no request).
5. **Scope.** Bounds and applications are resolved once and checkpointed with the run
   ID; recovery uses them. Test `recovery_keeps_the_original_scope` (the recovered run's
   archive target equals the checkpointed bounds).
6. **Cancellation at admission.** `ReadBudget` gains an admission gate checked before
   every send; the service's gate reads the durable job state. The watcher remains for
   in-flight interruption (its interval is `WATCHER_POLL_MS`). Test
   `a_cancellation_between_polls_admits_no_further_send` (watcher idled at 60 s,
   cancellation during the first delayed response: no further read).
7. **Fail closed.** `receipts_ingest` refuses `credential_blocked` before any
   reservation or archive effect unless the complete secret registry loads. Test
   `receipts_fail_closed_without_the_complete_secret_registry`.
8. **`history_status`.** Derived from the latest `collection_start` job, the
   collection schedule's backlog and the last 24 h of polled coverage. Test
   `history_status_reports_the_latest_collection_and_falling_behind`.

Minor: missed slots during downtime are inserted as `skipped` history (bounded to 672,
only where the version in effect was enabled). Test
`slots_missed_during_downtime_are_recorded_as_skipped`.

### M4 review 3 — 8.0/10, revise (preserved)

Source: `.local/dev/runtime-mcp/m4/review-3/result.md`.

The reviewer confirmed fixed: stale and partial polling progress, peer success masking a
denial (direct summaries), the receipt secret registry failing closed and
`history_status`. The cancellation admission gate passed in-memory.

Material findings:

1. Polling progress ignored application scope. An A/B selection advanced C and
   tenant-wide polling; an A traversal advanced an A/B request.
2. Crash recovery lost credential-denial and transient-retry decisions. A completed
   denied run started a new run without a changed revision (`waiting_retry` and a new
   reservation). A crash before the `continue` checkpoint after a transient token
   failure ended `partial`. Enrichment had the same structure.
3. First-collection recovery passed a null range, so the collector re-resolved the
   window from its own clock (one-hour shift).
4. Cumulative consumption omitted Reports sends (6 counted for 8).

Minor findings: downtime history used the current interval (one skipped slot instead of
three after 15 → 30 minutes); the service receipt path used the CLI's 250 ms retention
budget.

### M4 round 4 corrections (submitted for review 4)

Each fix was mutation-checked: reverting it makes its test fail.

1. **Scope-preserving polling.** `polling_lower_bound` interprets `scope_kind` and
   `selected_app_ids`: a `cloud` traversal covers every application, an `application`
   or `selection` traversal only the applications it names. Tenant-wide polling needs
   a cloud traversal; an application request needs progress for every requested
   application and takes the oldest. Test
   `polling_progress_keeps_its_application_scope`.
2. **Run-bound revision and reconciliation.** The checkpoint records the credential
   revision before any send. Recovery reconciles a found run once, for both executors:
   a credential denial is recorded under the bound revision, and the job parks
   `needs_credentials` with no new attempt unless the current revision differs; a
   transient token failure with no data parks `waiting_retry`. Tests
   `a_crash_after_a_denial_parks_the_job_without_a_new_attempt`,
   `a_crash_after_a_transient_token_failure_keeps_retry_eligibility` and
   `an_enrichment_crash_after_a_denial_parks_without_a_new_attempt`.
3. **Concrete bounds.** The collector is always given the checkpointed target; the
   unused nullable range is removed. Test
   `a_null_range_recovery_collects_the_checkpointed_target` (a naturally generated
   first-collection checkpoint). A first collection now reads its concrete target (24
   or 25 whole hours with the overlap); `a_later_tick_reads_only_since_the_last_traversal`
   accepts both.
4. **Reports in consumption.** Run consumption includes Reports sends, and job
   consumption adds the control store's durable `reports` reservations (uncertain
   included). Test `cumulative_traffic_includes_reports`.

Minor:

- Missed downtime slots follow each effective-dated version's interval, re-aligned at
  its effective time. Test `downtime_history_follows_each_versions_interval` (three
  skipped).
- The service receipt path writes under a 5 s service budget
  (`finish_received_within`). Qualifying it found a real defect: a 200-record batch was
  refused because its normalized size exceeded the 256 KiB write bound. Batches are now
  written in chunks within that bound, each its own transaction of the same
  deterministic run. Test
  `a_maximum_receipt_batch_fits_the_service_budget_and_replays_idempotently` (200
  accepted, identical replay, 200 receipts).

Also: Rust 1.89 Clippy findings fixed (`live.rs`, `signal/collect.rs`, `bulk/input.rs`,
`repository.rs`).

Known gaps, disclosed:

- Continuation after Reports traffic followed by a management denial is covered at
  unit level (run and job consumption), not by an end-to-end fixture.
- The enrichment reconciliation is tested for the denial path only; it shares
  `reconcile` with collection, whose retry path is tested.
- A budget timeout under heavy load is not reproduced deterministically; the test
  asserts a maximum batch succeeds and replays.

### M4 review 4 — 8.4/10, revise (preserved)

Source: `.local/dev/runtime-mcp/m4/review-4/result.md`.

The reviewer confirmed fixed: application/selection polling scope, finished-run denial
and transient recovery for both executors, null-range recovery, Reports in cumulative
consumption, the 15 → 30-minute downtime case and the receipt archive budget.

Material findings:

1. Polling progress came from `observed_ms`, not the traversal's target end: a cloud
   traversal of now−20h..now−12h observed now moved the next lower bound to now−59min.
2. Recovery of a `running` archive run returned before examining journaled denials, for
   both executors: a journaled 403 recovered `partial` with no denial record.
3. Recovery never restored a journaled management 429 hold (`cooldown=None` after a
   crash between archive finalization and the control-store hold), for both executors.

Minor finding: the missed-slot walk considered only the first version boundary (15-min,
then 60-min from 00:01, then 15-min from 00:10: zero skipped slots at 01:00, not three).

### M4 round 5 corrections (submitted for review 5)

Each probe is reproduced as a regression test, and each fix was mutation-checked:
reverting it makes its test fail.

1. **Target-end polling progress.** `polling_lower_bound` takes
   `min(until_ms, observed_ms)` per traversal. Test
   `historical_and_delayed_traversals_never_skip_unobserved_hours` (the reviewer's
   now−20h..now−12h probe gives now−12h−15min; a later traversal reaching now gives
   now−59min).
2. **Denials before the running shortcut.** `reconcile` checks journaled denials first,
   records the denial under the bound revision, closes a `running` run `interrupted`
   and parks `needs_credentials` with no attempt; a failed denial write still parks.
   Test `a_running_run_with_a_journaled_denial_parks_both_executors` (collection and
   enrichment; zero fixture requests).
3. **Recovered 429 holds.** `RunSummary` exposes `last_rate_limited_ms` (latest
   management refusal time from the journal); recovery installs the 15-minute hold from
   it through `Control::hold`, which keeps a longer existing hold. Tests
   `a_recovered_429_restores_the_management_hold_both_executors` and
   `mixed_denial_and_429_restore_both_on_recovery`.

Minor: `next_after` walks the effective-dated versions chronologically, reconsidering
each intervening version before accepting a candidate slot. Test
`downtime_history_walks_every_version_chronologically` (00:15, 00:30, 00:45 skipped).

Re-scan of recovery against the normal path: the run reservation (C2, `uncertain` on
restart), denials, transient retry, management 429 holds (now restored), Reports
charges and reservations (durable before each send), unconfirmed reads (from the
journal) and Reports 429s (durable in the archive's `service_cooldowns`, read before
work) are all reconstructed. No further gap was found.

### M4 review 5 — 8.6/10, revise (preserved)

Source: `.local/dev/runtime-mcp/m4/review-5/result.md`.

The reviewer confirmed the round-4 probes pass: target-end polling progress, a running
archive row with a journaled 403 (when executor reconciliation runs), restored 429
holds, mixed denial/429 and the chronological version walk.

Material findings:

1. Cancellation and deadline expiry bypassed credential-denial reconciliation: the
   scheduler pass (cancel, deadline) and the claim-time expiry finalized a recovered job
   before its executor reconciled the archive, and the normal executor returned
   `canceled` before recording a denial. For both executors, a journaled 403 in a
   running row followed by cancel or expiry left the revision unblocked, and the next
   collection reserved another attempt.
2. A failed denial write protected only the current job: with the `credential_denials`
   insertion failing, the first job parked, and after the fault cleared another job
   reserved an attempt with the rejected revision. Both executors.
3. Running-run recovery lost transient-token retry eligibility: a journaled OAuth 429 or
   503 gave `waiting_retry` for a finished run but `partial` for a `running` one. Both
   executors.

### M4 round 6 corrections (submitted for review 6)

Design: the archive journal is the authority for credential denial (contract §6.1);
the control-store record is its persisted copy and is never needed for safety.

- Each run is bound to its revision in the archive: `SyncRequest.credential_revision`
  is written into `collection_runs.provenance_json` with the run, before any send. The
  `collecting` checkpoint keeps its binding; `Control::bound_runs` reads it for runs
  recorded without the archive binding.
- `run_summary::revision_denied` reports whether any of the revision's 16 most recent
  bound runs (archive or checkpoint binding) holds a definitive denial. That is a 403;
  or a 401 with no later success of the same operation, once the run is finished or at
  once when the replay was refused too; or, in a finished run with no API data, any
  authentication refusal.
- `collection::revision_blocked` returns the control record OR the journal, writes the
  missing control record opportunistically, and errs (send nothing) when either store
  is unreadable. It is consulted at every admission point: `drive` (the first-send
  gate of every run of both executors) and `collection::credentials_rotated` (the
  reload requeue, factored out of `run.rs`).
- The worker tick runs a reconciliation pass (`revision_blocked` for the configured
  revision) before claiming, so a denial whose job was canceled or expired, or whose
  write failed, is persisted once the store accepts it. The job's terminal state is not
  touched.
- `drive` records the denial (and the 429 hold) before honouring a cancellation.
- `reconcile` classifies a transient token failure before the `running` shortcut: it
  closes the run `interrupted`, and the caller parks `waiting_retry` with the scope,
  the run (its consumption) and the original deadline.

Tests (each reproduces a reviewer probe; both executors unless noted):

1. `a_canceled_or_expired_job_keeps_its_state_and_the_revision_stays_blocked`. It covers
   cancel through the scheduler pass, expiry at claim and expiry in the scheduler pass,
   with checkpoint-only and archive-only binding. The job stays `canceled`/`failed`, the
   tick pass records the denial, and the next job of either executor ends
   `needs_credentials` with zero reservations and zero requests.
   - `admission_reads_the_journal_before_any_reconciliation`: the next job is refused
     with no control record and no tick pass.
   - `a_cancellation_during_a_denied_run_keeps_the_revision_blocked`: a live loopback
     run journals a 403, and the cancellation is observed at the next send. The job is
     `canceled`, the denial is recorded and the archive run carries its revision.
2. `a_failed_denial_write_still_refuses_every_job_until_reconciled`: an injected SQLite
   trigger aborts every `credential_denials` insert.
   - The first job parks, and other jobs of both executors are refused (zero
     reservations and requests).
   - The tick pass cannot record the denial while the fault lasts. Once it is dropped,
     the pass records it.
3. `a_running_run_with_a_transient_token_failure_waits_for_retry_both_executors`. It
   covers OAuth 429 and 503, in a `running` and a `partial` archive run. Each ends
   `waiting_retry` with the same deadline and reservations, and a `continue` checkpoint
   holding the original target and range, the run and reason `retry`. The running run
   is closed `interrupted`, and nothing is sent.

Two further tests:

- `the_journal_authority_classifies_denials_by_their_definitive_outcome` (classification
  table):
  - a 403 denies at once;
  - a single 401 denies only once its run is finished;
  - two refusals deny at once;
  - a recovered 401, an OAuth 503, a run of another revision and an unbound run never
    deny.
- `a_rotation_to_a_journal_denied_revision_resumes_nothing` (reload admission point).

Mutation checks (`collection` test target; each mutation applied alone, then reverted):

| Mutation | Killed by |
|---|---|
| Admission reads only the control store | `a_failed_denial_write…`, `admission_reads…` |
| No tick reconciliation pass | `a_canceled_or_expired…`, `a_failed_denial_write…` |
| Transient check after the `running` shortcut | `a_running_run_with_a_transient…` |
| Cancellation returns before the denial record | `a_cancellation_during_a_denied_run…` |
| Run not bound in archive provenance | `a_cancellation_during_a_denied_run…` |
| No checkpoint-binding fallback | `a_canceled_or_expired…`, `a_failed_denial_write…`, `admission_reads…` |
| A running single 401 counted as denial | `the_journal_authority…` |
| A 403 not definitive while running | `a_canceled_or_expired…`, `admission_reads…`, `the_journal_authority…` |
| Transient running run not closed | `a_running_run_with_a_transient…` |
| Reload requeues without the journal check | `a_rotation_to_a_journal_denied…` |

The first five mutations combined approximate the round-5 code, and fail all five
finding tests.

Admission and finalization audit:

- Management sends happen only inside `drive`, after `revision_blocked`. This covers:
  - collection fresh, continued, rotated and resumed (no archive row);
  - enrichment, the same cases;
  - Reports transports inside an admitted run.
- The other admission point is the reload requeue (`credentials_rotated`).
- Submissions (`collection_start`, `enrichment_start`, schedule ticks) send nothing.
  `receipts_ingest` sends nothing to AIRS.
- These finalizations never reach an executor, and the authority covers each:
  - claim-time recovery failure (unknown consumption);
  - claim-time deadline expiry;
  - scheduler-pass cancellation;
  - scheduler-pass deadline expiry (`waiting_retry`, `needs_credentials`,
    `blocked_policy`);
  - post-run cancel-checkpoint release.
- The executor-side finalizations are the `canceled` return, which now follows the
  denial record, and recovery's Parked, Retry, Rotated and Finish.

Remaining limits:

- A checkpoint written before checkpoints carried a revision (pre-M4-round-3 builds)
  binds nothing for the authority. Its own recovery still assumes the configured
  revision.
- A run left `running` with a single unanswered 401 is not treated as a denial.
- The reconciliation pass persists only the configured revision; other revisions are
  checked when a reload makes them current.

### M4 review 6 — 8.7/10, revise (preserved)

Source: `.local/dev/runtime-mcp/m4/review-6/result.md`. All 656 manifest entries were
verified before and after the review; the snapshot was unchanged.

The reviewer confirmed every review-5 probe passes: a running API 403 then cancel or
expiry (both executors, both bindings), a failed denial insert followed by another job,
and a running OAuth 429/503 (retry eligibility, scope and consumption preserved).

Material findings:

1. Cancellation or expiry can still bypass a journaled OAuth denial:
   `run_summary.rs` recognises OAuth authentication refusals only once the archive run
   is no longer `running`, and cancellation or deadline finalization can leave it
   running for good. A running row holding an OAuth 401 or 403, then cancel or
   claim-time expiry, left `revision_blocked=false`, and another job reserved a fresh
   attempt. Both executors.
2. The 15-minute management hold can disappear after cancellation, expiry or a failed
   hold write: `collection.rs` ignores control-hold write failures, the reconciliation
   pass restores denials but not holds, and the archive cooldown honours a shorter
   `Retry-After`. With a 429 two minutes earlier and `Retry-After: 1`, another run sent
   while 13 minutes of the service hold remained. Both executors.
3. Credential-blocked jobs can stay stranded after rotation: `credentials_rotated` runs
   only when SIGHUP observes a revision change. Startup with a new revision, or a failed
   resumption write, left jobs `needs_credentials` across three worker passes. Both
   executors.

No separate minor findings were confirmed. Deferring report-to-period `enrichment_start`
and daily schedule execution to M6 remains an accepted scope boundary.

### M4 round 7 corrections (submitted for review 7)

Design: round 6 made the archive journal the authority for a revision's denial. Round 7
extends the same rule to the two other outcomes that a terminal path could lose: the
management hold and credential resumption. Each is derived from durable state at every
admission and every worker pass, never only on an executor's completion or a SIGHUP edge.

1. **Token refusals are definitive while running.** `run_summary::revision_denied`
   counts an `oauth.token` 401 or 403 with no token returned later in the same run
   (`TOKEN_TERMINAL`), whatever the run's state. The collector never replays a token
   refusal, so nothing pending can answer it. A token started in the same millisecond
   does not count, so ambiguity keeps the refusal. The summary carries
   `token_terminal_denied`, and the executor's `credential_denied` uses it too, so a
   recovered `running` run with API data and a token refusal parks as well.
2. **The management hold is derived from the archive at admission.**
   `run_summary::management_hold_until` is the latest journaled API 429 plus 15 minutes,
   or the archive's management `service_cooldowns.until_ms`, whichever ends later.
   - The journal scan is bounded to rows started within the hold plus a one-hour read
     window, read through the `management_reads_clock` index.
   - `collection::management_hold` is the longer of that and the control cooldown. It
     replaces the control-only `cooldown_until` in `drive`, which is the first-send gate
     of both executors, and copies a missing hold into the control store.
   - A short provider `Retry-After` therefore no longer shortens the service hold, and a
     longer one is kept.
   - The worker's reconciliation pass restores the control hold. Failed hold writes in
     `drive` and recovery are logged as `management_hold_deferred`.
3. **Credential resumption happens at every pass.** Parking (`Control::park_credentials`,
   the worker's `Outcome::NeedsCredentials { revision }`) records the revision the job was
   denied under in `jobs.credential_revision`, in the same transaction as the edge.
   - `credentials_restored` requeues only jobs parked under another revision, or under
     none recorded. Each job's edge and revision commit in one transaction.
   - `run::reconcile_pass` runs once at worker startup, before the first tick, and at
     every tick. It covers the denial record, the hold record and
     `collection::credentials_rotated` for the configured revision. The SIGHUP path is
     kept.
   - A startup with a new revision, or a failed resumption write, is therefore retried.
     A job parked under the configured revision stays parked, with no requeue churn. A
     denied revision resumes nothing. Deadline, checkpoint and consumption are
     untouched.

Tests (both executors in every case):

1. `a_running_token_refusal_blocks_the_revision_after_cancel_or_expiry` covers 24 cases:
   OAuth 401 and 403, checkpoint-only and archive-only binding, and cancel, expiry at
   claim and expiry in the pass. The journal blocks the revision with no executor and no
   pass, the pass records it, the terminal job keeps its state, and the next job of
   either executor makes no reservation and no request.
   - `a_running_token_refusal_after_api_data_parks_both_executors` covers the executor
     recovery path.
   - The classification table gains four cases: running OAuth 401 → denied, running
     OAuth 403 → denied, a running token refusal after API data → denied, and a running
     OAuth 429 → not denied.
2. `the_management_hold_survives_cancel_expiry_and_a_failed_hold_write` covers 16 cases.
   It seeds an API 429 two minutes earlier with its companion archive cooldown at
   `Retry-After: 1`, then applies cancel, either expiry path, or recovery with every
   `cooldowns` insert and update aborted by an injected trigger.
   - The next job of either executor waits (`waiting_retry`, retry at least 12 minutes
     away) with no reservation and no request.
   - The pass restores the control hold at exactly the refusal time plus 15 minutes, and
     only once a failing write clears.
   - `the_management_hold_keeps_a_longer_provider_cooldown` checks that a 429 sixteen
     minutes old with `Retry-After: 1` holds nothing, and that `Retry-After: 3600` holds
     for the hour.
3. `parked_jobs_resume_at_startup_and_after_a_failed_resumption` covers the startup
   reconciliation with a new, undenied revision and no reload edge, and an injected
   abort of the `needs_credentials → queued` update across three passes, then cleared.
   - Passes under the parking revision, and under a journal-denied revision, resume
     nothing.
   - The resumed job keeps its deadline, checkpoint and consumption. Another pass is
     idempotent.
   - `a_job_parked_under_the_configured_revision_stays_parked_until_a_rotation` parks a
     job whose secret file is unreadable, so no denial is journaled. Three passes add no
     job events, and a new revision resumes it.

Mutation checks (whole `collection` target, each mutation applied alone, then reverted;
`.local/dev/runtime-mcp/m4r7/mutation-check.py`, log alongside):

| Mutation | Killed by |
|---|---|
| Token refusal not definitive while running | `a_running_token_refusal_blocks…`, `the_journal_authority…` |
| Executor ignores a definitive token refusal | `a_running_token_refusal_after_api_data…` |
| Admission reads the control cooldown only | `the_management_hold_survives…` |
| Archive hold ignores the journaled 429 (Retry-After only) | `the_management_hold_survives…` |
| No hold restoration in the pass | `the_management_hold_survives…` |
| No credential resumption in the pass (SIGHUP only) | `parked_jobs_resume…`, `a_job_parked_under_the_configured_revision…` |
| Resumption ignores the parked revision | `a_job_parked_under_the_configured_revision…` |
| Parking does not record its revision | `a_job_parked_under_the_configured_revision…` |
| Round 6: admission reads the control denial only | `a_failed_denial_write…`, `admission_reads…` |
| Round 6: no reconciliation pass | five tests, including both new pass tests |
| Round 6: running single API 401 counted as denial | `the_journal_authority…` |
| Round 6: API 403 not definitive while running | `a_canceled_or_expired…`, `admission_reads…`, `the_journal_authority…` |

Sibling audit:

- **Every outcome a terminal path could lose** is now derived at admission and restored
  by the pass: the denial (round 6), the hold, and resumption.
  - The finalizations that never reach an executor are the same as in round 6: claim-time
    recovery failure, claim-time expiry, scheduler-pass cancel and expiry, and the
    post-run cancel checkpoint.
  - Each is covered for all three outcomes. For resumption, a terminal job is not parked
    and so has nothing to resume.
- **Token plane.** An OAuth 429 writes the archive's management cooldown, 900 s without
  a `Retry-After`. The service now waits for it at admission instead of starting a run
  the archive would refuse. Transient token retry eligibility is unchanged; the wait is
  longer.
- **Reports plane.** A Reports 429 stops the run and writes the archive's `reports`
  cooldown, which the collector checks before each Reports transport. The contract
  documents no service-level Reports hold. Admission is per transport, under a durable
  allowance, with no executor-dependent state, so there is no gap of this shape.
- **Scanner plane.** `receipts_ingest` sends nothing to AIRS.

Remaining limits:

- A token refusal and a later successful token of the same run in the same millisecond
  are read as a refusal (conservative). The "later token" exception is tested only by
  inspection, because the seeding helper uses one timestamp per run.
- The startup pass is exercised through `startup_reconcile_for_tests`, which is the same
  function `run_worker` calls first. No full service restart with listeners was run.
- A job parked before this round has no recorded revision, so it is resumed once by the
  first pass. If its revision is still undenied, it runs once and parks again with its
  revision recorded.
- The hold reads the archive at each admission: one indexed range query and one
  single-row lookup.
- Checkpoints from before M4 round 3 still bind nothing (as in round 6).

### M4 review 7 — 8.8/10, revise (preserved)

Source: `.local/dev/runtime-mcp/m4/review-7/result.md`. All 656 manifest entries were
verified before and after the review; the snapshot was unchanged.

The reviewer confirmed every review-6 probe passes (running OAuth 401/403 then cancel or
either expiry; short `Retry-After` with cancel/expiry/failed hold writes at the next
run's admission; startup rotation and failed resumption), and the review-5 probes.

Material findings (both are admission checked only at run start, not per send inside an
active run; both executors):

1. An active run can send during the 15-minute management hold: `sync.rs` checks the
   archive cooldown, which `Retry-After` can shorten, and the service-level hold is
   checked only at run admission. With a 429 carrying `Retry-After: 1` while admitted
   peer reads were active, the collector sent a new sessions-overview request about 1.4 s
   after the refusal.
2. An active run can request another token after a definitive OAuth denial: revision
   denial is checked only at run admission, so after a journaled `oauth.token` 401/403
   the collector sent a second token request with the unchanged revision.

No separate minor findings were confirmed.

### M4 round 8 corrections (submitted for review 8)

Design: run-start admission is not enough. One in-run transport fence, kept in the
archive journal's per-transport admission (`sync.rs`, `ReadJournal::begin`), now
reads durable state before every management and OAuth send of a service run. It
decides in the transaction that records the transport `started`, so the decision and
the record cannot diverge. It is service-scoped (`SyncRequest.send_fence`, set by
`collection::drive` for both executors). The CLI-equivalent history driver passes
`None` and keeps its behaviour.

1. **Hold (review-7 finding 1).**
   - A service run's API 429 now commits the archive's management cooldown in the
     same transaction as the 429. It is the provider's `Retry-After` raised to at
     least the service's 15 minutes; a longer provider duration is kept.
   - The existing per-transport cooldown check therefore refuses every later send of
     the run, of its peers, and of every other run, until the hold ends. A peer read
     already in flight finishes; nothing is sent after it.
   - A token-endpoint 429 keeps the provider's duration. The contract's hold applies
     to API 429s, and the transient-token retry rules are unchanged.
2. **Credential fence (review-7 finding 2).**
   - `sync::run_definitively_denied` is the running-run subset of
     `run_summary::revision_denied`, sharing `TOKEN_TERMINAL`. It matches a token
     401/403 with no later token, an API 403, or an API read refused twice with no
     later success.
   - When the run's journal holds such a refusal, the journal refuses every later
     transport of the run, token and API alike.
   - A single API 401 is not definitive, so its one accounted fresh token and replay
     are still admitted. Job classification is unchanged: the job parks
     `needs_credentials` from the run's summary.

Tests (`crates/airs-mcp/tests/collection.rs`; both executors):

- `an_active_run_admits_no_send_during_the_management_hold_both_executors`: the
  reviewer's probe.
  - The profiles walk gets a 429 after 100 ms with `Retry-After` 1, 900 or 3600.
  - The customer-app peer read was admitted before the 429 and answers after 1.5 s.
  - Asserted: one profiles read, one peer read and zero sessions-overview reads; an
    archive cooldown of max(15 min, provider duration); the next job of either
    executor waits with no reservation or request.
  - On the round-7 source this fails, with a sessions-overview read sent after the 429.
- `an_active_run_sends_nothing_after_a_definitive_token_refusal_both_executors`: an
  OAuth 401 and an OAuth 403.
  - Asserted: exactly one token request and no API request; the job is
    `needs_credentials`; the revision is blocked.
  - On the round-7 source this fails, with two token requests.
- `the_fence_keeps_the_single_fresh_token_replay_both_executors`:
  - one API 401 gives two overview reads and two tokens, completes, and blocks
    nothing;
  - a refused replay is definitive: two overview reads, then no session read, and
    the job is `needs_credentials`.

Mutation checks (`m4r8/mutation-check.py`, log `m4r8/mutation-check.log`). Each
mutation was applied alone, the `collection` test target run, then reverted. The
round-7 and round-6 mutations were re-run and stay killed.

| Mutation | Killed by |
|---|---|
| P1 service run without the credential fence | the token-refusal test |
| P2 no service hold floor with the 429 | the active-hold test |
| P3 journal admission skips the denial fence | the token-refusal test |
| P4 fence ignores token-endpoint refusals | the token-refusal test |
| P5 fence treats a single API 401 as definitive | `a_401_gets_exactly_one…`, the replay test |
| P6 floor replaces a longer provider cooldown | the active-hold test (`Retry-After: 3600`) |

The round-7 mutations N1–N8 and the round-6 mutations M1, M2, M7 and M8 stay killed by
their original tests.

Changed test: `a_cancellation_during_a_denied_run_keeps_the_revision_blocked` (round 6).

- Its setup relied on a run that journals an API 403 and goes on to a later slow read.
  An API 403 now fences every later send, so that read is never sent. The first
  mutation run failed this test under every mutation, which is how the conflict was
  found.
- The setup now uses a concurrent customer-app read, admitted before the 403 and
  still in flight when the cancellation commits.
- The assertions are unchanged: the job is `canceled`, the denial is recorded, the
  archive binds the run's revision, and the next job makes no attempt.
- The test passed in six repeated runs.

Behaviour change: an API 403 now also stops the rest of its run. A 403 already
denied the revision for later runs (`revision_denied`), so this only removes sends
the job could not use.

Send-path audit:

- Every management and OAuth transport of both executors passes through
  `BoundedTransport` and so through the journal's `begin`. That covers the
  configuration walks (concurrent `join3`), discovery, session and transaction
  reads, the token exchange and the API-401 replay.
- `airs.scan.*` (Reports) transports are not journaled. They use the scanner
  credential and keep their own gates: the durable daily allowance, two sends and ten
  IDs per job, and a stop at the first 429.
- Run-start admission (`drive`: the hold and `revision_blocked`), the reload requeue
  and the worker pass are unchanged.

Remaining limits:

- A transport already admitted before the refusal or the 429 was committed still
  completes. That is an in-flight send, not a later one: the fence cannot recall it.
- An API 401 is never fenced on its own. Concurrent peers whose lease expired
  together may each make their one replay.
- The tests use the loopback fixture's per-route delays, not the reviewer's in-memory
  transport. Full service restarts and socket-level cancellation were not exercised.

### M4 review 8 — 8.8/10, revise (preserved)

Source: `.local/dev/runtime-mcp/m4/review-8/result.md`. All 656 hashes and the exact file
inventory matched before and after; the snapshot was unchanged.

The reviewer confirmed both review-7 findings are fixed for both executors (no management
send after a short-`Retry-After` 429 from delayed peers; exactly one token request after an
OAuth 401/403), the legitimate single API-401 replay still works, and all 24 earlier
recovery probes pass.

Material findings (both executors):

1. Distinct transaction reads are mistaken for one refused replay: `read_budget.rs`
   omits `scan_id`/`scan_sub_req_id` from journal descriptors, and `sync.rs` treats two
   refusals with matching descriptors as a refused replay. Two different transaction reads
   each getting a first 401 parked the job `needs_credentials` and blocked a valid
   revision.
2. Scanner 401s get a management fresh-token replay with incomplete accounting:
   `read_budget.rs` replays any HTTP 401, and scanner Results and Reports use that
   wrapper (`scan_logs.rs`). Collection Results and enrichment Results sent twice with the
   unchanged scanner credential and recorded one send; enrichment Reports recorded two
   reservations but one reported send.

No separate minor findings were confirmed.

### M4 round 9 corrections (submitted for review 9)

Design: each logical read is one replay chain, and only a management read is replayed.
Both are decided in the shared read budget (`airs-runtime-core/src/read_budget.rs`), so
every judgement that compares journal rows — the in-run fence, run summaries, the
revision authority and executor recovery — inherits them from the descriptor itself.

1. **Replay chain (review 8 finding 1).** `ReadBudget::run` opens a chain for each
   logical read (a task-local, scoped to that read's future): its first attempt, its
   single replay and any OAuth request either needs carry the same `replay_chain` in
   the journal descriptor, and distinct reads never share one. The descriptor also
   keeps the read's full logical identity: `scan_id` and `scan_sub_req_id` joined the
   allow-listed query keys. Every descriptor comparison (`sync::run_definitively_denied`,
   `run_summary` `terminal_denied` and `revision_denied`) is therefore within one chain:
   "a read refused twice" is a read and its own refused replay, and "a 401 followed by
   a success" is answered only by its own replay.
2. **Management-only replay (review 8 finding 2).** The replay fires only when a
   management API transport of the chain answered 401 (`Chain::management_401`, set by
   the bounded transport for operations other than `oauth.token` and `airs.scan.*`).
   A scanner Results or Reports 401 (a static API key) is final: one transport, no
   OAuth renewal.
3. **Results accounting.** `run_summary` counts Results by distinct `transport_id`
   (one admitted batch of at most five IDs is one transport), as Reports already did.
   It counted work attempts (five per transport), and before (2) it missed the
   replayed send. The run's durable count, the job result's `sends.results` and the
   job's consumption now equal the transports sent.

The CLI is unaffected: the fresh-token replay is enabled only by the service's sync
(`enable_auth_replay`), and the CLI does not journal. `AIRS_REQUIRE_PARITY=1 cargo test
--workspace` passes: 4,032 passed, 0 failed, 5 ignored (`m4r9/full.log`).

Tests (`crates/airs-mcp/tests/collection.rs` unless noted):

- `distinct_transaction_reads_each_keep_their_own_fresh_token_replay_both_executors`
  reproduces the reviewer's probe. Two transaction reads differ only by `scan_id`, or
  only by `scan_sub_req_id` (collection), or are two of the enrichment session's five
  scans. Each gets a first 401 (10 and 20 ms) while the fresh tokens take 40 ms.
  - Each read sends once more as its own replay (four, or seven, transaction sends),
    the job is not parked, and the revision stays usable.
  - When every replay is refused too, the job parks `needs_credentials`, the revision
    is blocked, and at most one replay per read was sent.
- `the_journal_authority_classifies_refusals_by_replay_chain`: two running 401s of
  distinct chains are not definitive; a read and its refused replay are; each read
  answered by its own replay is not a denial; another read's success never answers a
  refusal it did not replay.
- `read_budget::replay_chain_tests` (core, real SDK against the loopback fixture):
  - `each_logical_read_and_its_replay_share_one_chain_and_distinct_reads_never_do`: two
    concurrent reads with an *identical* query each get a first 401. Four transports,
    two chains, each chain a read and its replay; every transaction descriptor keeps
    the session, application, scan and sub-request.
  - `a_scanner_401_is_never_replayed_even_with_the_service_replay`: a Results 401 is one
    transport, no replay, no token.
- `a_scanner_401_is_one_accounted_transport_both_executors`: a Results 401 (collection,
  enrichment) and a Reports 401 (enrichment) with the service replay enabled: one
  transport, one token request, the run's durable Results/Reports counts equal the
  transports, `sends` in the job result equals them, one Reports reservation per
  transport, and the management revision is not blocked.

Mutation checks (`m4r9/mutation-check.py`, log `m4r9/mutation-check.log`; each applied
alone to the committed tree, the `collection` target run, then reverted):

| Mutation | Killed by |
|---|---|
| Q1 no replay chain and no scan identity (the round-8 descriptor) | the distinct-reads test, the core chain test |
| Q2 no replay chain (scan identity kept) | the core chain test |
| Q3 no scan identity (replay chain kept) | the core chain test (full identity) |
| Q4 replay not restricted to a management 401 | the scanner-401 tests (core and both executors) |
| Q5 scanner reads eligible for the replay | the scanner-401 tests (core and both executors) |
| Q6 Results accounted per work attempt | the both-executor scanner-401 test |
| Q7 in-run fence matches refusals by operation only | the distinct-reads test |
| Q8 revision authority matches refusals by operation only | the chain classification test |
| P1–P6 (round 8), N1–N8 (round 7), M1, M2, M7, M8 (round 6) | their original tests, still killed |

The first run (`mutation-check-run1.log`) had Q2 and Q3 surviving: the replay chain and
the scan identity each separate the fixture's distinct reads alone. The core tests were
added so that each layer is pinned on its own; the final run kills all 26.

Audit:

- **Descriptor comparisons.** Every comparison of journal rows uses
  `descriptor_json` equality, now chain-aware: the in-run fence, `terminal_denied`,
  `revision_denied` and the executor's `credential_denied` (from the summary). The
  scanner work descriptors (`work_attempts`) already carry the full scan identity.
- **Transports through the replay wrapper.** Management reads (journaled, chain,
  replay-eligible), `oauth.token` (journaled, chain, never replayed), scanner Results
  and Reports (not journaled, never replayed). The backlog revisit path
  (`sync_backlog.rs`) uses the same wrapper and reservation.
- **Accounting per send class.** Management and OAuth: one journal row per transport
  (a replay is its own row). Results: distinct transport per admitted batch. Reports:
  distinct transport, plus a control reservation per transport. The job's
  consumption sums these and every unconfirmed read.

Remaining limits:

- Journal rows written before round 9 carry no `replay_chain` and no scan identity.
  A run recovered from such an archive still compares those rows by operation and the
  older query keys.
- An OAuth refresh shared by concurrent reads (the SDK's single-flight lease) is
  journaled under the chain of whichever read polled it. Token outcomes are classified
  by source, not by chain, so this does not change any judgement.
- The tests use the loopback fixture's per-route delays and FIFO one-shot responses,
  not the reviewer's in-memory transport. Full service restarts were not exercised.

### M4 review 9 — 9.0/10, revise (preserved)

Source: `.local/dev/runtime-mcp/m4/review-9/result.md`. All 656 hashes verified before and
after; the snapshot was unchanged.

The reviewer confirmed both review-8 findings fixed for both executors (distinct scan IDs
and sub-requests each get their own replay; scanner Results/Reports 401s send once with
journal, reservations, results and consumption in agreement, including five-ID batches),
concurrent identical queries across all seven management API paths, and every review-5–7
probe, traffic-limit probe and C2–C4 case.

Material finding:

1. A committed scheduled job can permanently lose its slot history:
   `schedule_store.rs` commits the job through `submit` and then separately inserts its
   slot row and sets `jobs.scheduled_slot` (the job insert leaves it null). With the slot
   insert failing after the submission committed, a reopen in the next quarter-hour, the
   job completing and another tick, the original slot had no history row and the job kept
   `scheduled_slot=NULL`. Lost attribution, not duplicate traffic; the gap predates
   round 9.

No separate minor findings were confirmed.

### M4 round 10 corrections (submitted for review 10)

- **The slot association commits with the submission.** `Submission.scheduled_slot`
  carries a scheduled job's `(schedule, slot)`. `submit` writes the job with its
  `scheduled_slot` and associates the slot row (`Control::associate_slot`) in the same
  transaction as the job, its parameters, its idempotency record and its first event.
  The separate post-submission writes in `schedule_store.rs` are removed. A same-key
  retry also re-associates, inside its own transaction.
- **The due slot is recorded before submission.** The tick's own transaction records
  the due slot `pending`. A submission that fails rolls back whole (no job, no
  idempotency record), and the slot stays in history: the next tick in a later slot
  records it `skipped`, never fetched; a retry in the same slot submits and takes the
  row over.
- **Stranded jobs are repaired.** Every tick first runs
  `repair_slot_associations`: a scheduler collection job with no `scheduled_slot`
  (a store written by the earlier two-step submission) is re-associated from its
  `sched-collection-<slot ms>` idempotency key, as `submitted`, or `completed` if the job
  is already terminal, even after its slot has passed. A slot row already naming another
  job is left alone.

Tests (`schedule_store.rs`):

- `a_failed_slot_association_commits_no_job_and_keeps_the_slot_in_history` (the
  reviewer's probe): injected triggers fail the association; the tick errs and commits
  no job or idempotency record; the slot stays `pending`. After a reopen in the next
  quarter-hour the new slot submits, the failed slot is `skipped`, and after the job
  finishes and another tick, its slot is `completed`. Every scheduler job names its slot
  and every such slot names its job.
- `a_retry_in_the_same_slot_after_a_failed_association_submits_and_associates`.
- `a_job_stranded_without_its_slot_is_reassociated_after_the_slot_has_passed`: a job
  submitted the old way is re-associated at the next quarter-hour (while live, so the
  new slot coalesces), its slot completes when it finishes, and a job already terminal
  when repaired is recorded `completed`; the repair is idempotent.

Mutation checks (`m4r10/mutation-check.py`, log `m4r10/mutation-check.log`):

| Mutation | Result |
|---|---|
| R1 association outside the submission (the earlier two-step, second write lost) | killed by all three tests |
| R2 no repair of stranded jobs | killed by the stranded-job test |
| R3 due slot not pre-recorded | killed by the failed-association test |
| R4 `scheduled_slot` not set in the job insert | survived: equivalent, because `associate_slot` sets it in the same transaction |
| R5 repair records a terminal job `submitted` | killed by the stranded-job test |

Limits:

- The same-key re-association on the idempotent path is defence in depth; no test
  isolates it, because the atomic submission leaves nothing for it to repair.
- Repair recognises only the scheduler's own `sched-collection-<ms>` keys.
- No real process kill was exercised; failures are injected with SQLite triggers on one
  connection, and restarts are a store reopen.

### M4 review 10 — 9.0/10, revise (preserved)

Source: `.local/dev/runtime-mcp/m4/review-10/result.md`. All 656 hashes and the exact
inventory matched after the review; the snapshot was unchanged.

The reviewer confirmed the review-9 schedule correction (38 injected failure and recovery
cases across submission, association, tick bookkeeping, legacy repair and same-key
repair, including both legacy two-step crash states) and every earlier worker, recovery,
transport and C2–C4 probe.

Material finding:

1. Receipt batch identity omits the recovery epoch: `tools.rs` derives the archive run ID
   from tenant, caller and key, while the control-store idempotency scope
   (`ingest_store.rs`) includes the epoch. After a backup, restore and activation, a new
   batch reusing the key text is accepted by the new control scope but merged into the
   old archive run: a one-record batch reported `accepted: 2`, or `accepted: 201` after a
   200-record batch (above the schema maximum of 200). Old-epoch requests are correctly
   refused. The gap predates round 10.

No separate minor findings were confirmed.

### M4 round 11 corrections (submitted for review 11)

- **The receipt run is named by the batch key's complete idempotency scope.**
  `tools::receipt_run_id` derives the archive run from a canonical JSON tuple of the
  scope the control store uses (tenant, caller subject and client, recovery epoch, key),
  so the same key text under a later epoch is a different batch with its own run, and no
  field can run into another.
- **Batches left pending by an earlier build still recover into their run.** Every
  submission first reserves its key (a fresh reservation is `pending`). For a pending
  batch, the epoch-less run ID earlier builds used is adopted only when that run started
  at or after this reservation (`Control::ingest_reserved_ms`,
  `run_summary::run_started_ms`): a batch the earlier build reserved and wrote, then
  crashed. An epoch-less run of the same key text from an earlier epoch predates the
  new reservation and is never adopted.

Tests (`tools.rs`):

- `a_batch_key_reused_after_a_restore_is_its_own_batch_and_run`: a 200-record batch,
  stored under the epoch-less run ID as an earlier build would have; a new epoch is
  activated from `pending_activation`. The old-epoch key is `control_restored`. The
  same key text under the new epoch, reserved by a process that died before writing,
  reports `accepted: 1, duplicates: 0` (not 2 or 201), a replay returns the same result,
  and the archive holds 201 receipts in 2 runs. Field-separation and epoch-separation of
  the run ID are pinned.
- `a_pending_batch_from_an_earlier_build_recovers_into_its_run`: a batch written under
  the epoch-less run ID with its reservation left pending recovers with
  `accepted: 2, duplicates: 0`.

Mutation checks (`m4r11/mutation-check.py`, log `m4r11/mutation-check.log`; all killed):

| Mutation | Killed by |
|---|---|
| S1 run ID without the epoch (the review-10 code) | `a_batch_key_reused_after_a_restore…` |
| S2 a pending earlier-build batch not recovered into its run | `a_pending_batch_from_an_earlier_build…` |
| S3 no reservation-time guard (any epoch-less run adopted) | `a_batch_key_reused_after_a_restore…` |

Limits:

- The epoch change in the tests inserts a `pending_activation` epoch and activates it
  through `Control::activate_epoch`; the reviewer's full backup, restore and reopen
  procedure was not re-run here.
- The legacy guard compares wall-clock milliseconds of the reservation and the run; a
  clock stepped backwards between them could make a same-epoch pending batch from an
  earlier build re-count under the new run (counts only; no receipt is written twice).

### M4 review 11 — 9.0/10, revise (preserved)

Source: `.local/dev/runtime-mcp/m4/review-11/result.md`. All 656 hashes and the exact
inventory matched before and after; the snapshot was unchanged.

The reviewer confirmed the review-10 probe passes (after a one- or 200-record batch, the
reused key under the new epoch reports `accepted: 1` in its own run), normal pending
recovery of current and legacy IDs, distinct collection and enrichment runs after a
restore, and every earlier probe.

Material findings (both in round 11's legacy recovery):

1. A timestamp comparison does not establish legacy batch ownership: with persisted
   timestamps modelling a clock rollback, a fresh new-epoch batch adopted the earlier
   epoch's epoch-less run (`accepted: 2`, or `201`), and a same-epoch pending legacy batch
   switched runs (`accepted: 0, duplicates: 2`).
2. Legacy lookup errors silently selected the new identity: with the archive lock held
   past the 2 s lookup, the call succeeded under a second run with
   `accepted: 0, duplicates: 2` and persisted that result.

No separate minor findings were confirmed.

### M4 round 12 corrections (submitted for review 12)

Round 11's inferred legacy ownership (a timestamp comparison and an archive lookup) is
removed. Ownership is now an explicit, durable binding.

- **The reservation binds the batch's run.** `Control::ingest_reserve` writes the
  pending reservation with its archive run and identity format
  (`{"pending": true, "run": {"id", "format": 2}}`, the run named by the key's complete
  scope including the epoch) in the same write that reserves the key and binds its
  digest, before any archive effect. Every later attempt of that batch, including
  recovery after a crash, runs in the bound run.
- **Earlier-build reservations migrate, durably, before any effect.** A pending
  reservation without a binding can only have been written by an earlier build, which
  wrote under its epoch-less run ID; `ingest_reserve` binds it to that run (format 1) in
  the same transaction, scoped to the same digest. A reservation of another epoch is
  never touched, so an earlier epoch's batch is never adopted.
- **No guessing.** The tool uses only the bound run; a reservation without one (which
  `ingest_reserve` cannot return) is refused `storage_unavailable` with no archive
  effect. No clock is compared and no archive read decides ownership, so neither clock
  rollback nor lock contention can change which run a batch writes.

Tests (`tools.rs`):

- `a_batch_key_reused_after_a_restore_is_its_own_batch_and_run`: the earlier epoch's
  200-record batch stored under the epoch-less ID; after the epoch change, a new-epoch
  batch reserved with an arbitrary reservation time and recovered as pending reports
  `accepted: 1, duplicates: 0`; 201 receipts in 2 runs.
- `a_pending_batch_from_an_earlier_build_recovers_into_its_run`: an unbound pending
  reservation is bound to the earlier build's run (format 1, durable before any write)
  and recovers with `accepted: 2, duplicates: 0`.
- `receipt_batches_reserve_first_replay_stably…`: its crash fixture now leaves the
  reservation with the binding this build writes.

Mutation checks (`m4r12/mutation-check.py`, log `m4r12/mutation-check.log`; all killed):

| Mutation | Killed by |
|---|---|
| T1 run ID without the epoch | `receipt_batches_reserve_first…` |
| T2 earlier-build reservation not migrated | `a_pending_batch_from_an_earlier_build…` |
| T3 migration binds the new run | `a_pending_batch_from_an_earlier_build…` |
| T5 fresh reservation binds no run | `receipt_batches_reserve_first…`, `a_batch_key_reused_after_a_restore…` |

Limits:

- The epoch change in the tests activates a `pending_activation` epoch through
  `Control::activate_epoch`; the full backup, restore and reopen was not re-run here.
- Contention on the archive write is bounded by the existing 5 s receipt budget and
  returns `storage_unavailable` with the reservation kept pending and bound.

### M4 review 12 — 9.0/10, revise (preserved)

Source: `.local/dev/runtime-mcp/m4/review-12/result.md`. All 656 hashes passed before and
after; the inventory matched exactly.

The reviewer confirmed the review-11 probes pass (fresh round-12 reservations under both
rollback cases; legacy recovery with the archive lock held past the former lookup
budget), that bindings survive failed archive writes and reopen, and every earlier
probe.

Material findings:

1. Unbound reservations do not prove legacy ownership: `ingest_store.rs` binds every
   unbound pending reservation to the epoch-less run, but round-11 builds also wrote
   unbound reservations for epoch-scoped runs. A pending round-11 two-record batch switched
   runs (`accepted: 0, duplicates: 2`); after an actual backup, restore, activation and
   reopen, an unbound new-epoch reservation adopted the previous epoch's 200-record run
   (`accepted: 201`). The round-12 ledger claim that an unbound reservation "can only
   have been written by an earlier build, which wrote under its epoch-less run ID" was
   wrong: round 11 wrote unbound reservations too.
2. Receipt counts still use wall-clock ordering as ownership evidence:
   `run_summary::receipt_run_counts` decides acceptance by `started_ms`, so with
   persisted timestamps modelling a rollback a later duplicate reported `accepted: 2`
   and recovering the original batch changed its result to `accepted: 0, duplicates: 2`.

No separate minor findings were confirmed.

### M4 round 13 corrections (submitted for review 13)

- **Counts come from an immutable first-acceptance record.** A service receipt run's
  `provenance_json` carries `first_accepted`, and the transaction that inserts a receipt
  the archive did not hold appends its ID there (`repository::retain_chunk`).
  `run_summary::receipt_run_counts` counts a run that has this record from it alone:
  accepted = its receipts it first accepted, duplicates = the rest. No run timestamp
  decides ownership. Runs written without the record (CLI runs, older archives) keep
  the earlier derivation.
- **Unbound reservations are bound only from proven provenance.** `ingest_reserve` no
  longer binds an existing unbound reservation. `receipts_ingest` resolves it
  (`resolve_unbound_receipt_run`) and persists the binding with `Control::ingest_bind`
  before any archive effect:
  - the scope-named run exists: that run (its ID is a hash of the complete scope, so no
    other batch can own it), even if an epoch-less run of the key also exists;
  - neither run exists: nothing was written, so the scope-named run;
  - only the epoch-less run exists: it is this reservation's only when the store never
    had a restore (one recovery epoch); otherwise ownership is ambiguous and the batch
    is refused `conflict` with no archive effect, its reservation left unbound for
    explicit reconciliation;
  - a lookup failure is `storage_unavailable`, with nothing bound or written.
- **Ledger correction.** The round-12 entry's claim that an unbound reservation "can only
  have been written by an earlier build, which wrote under its epoch-less run ID" was
  wrong: round-11 builds wrote unbound reservations under the scope-named run.

Tests (`tools.rs`):

- `receipt_counts_do_not_depend_on_run_timestamps`: two batches of the same two receipts
  (2/0, then 0/2); the first run's start moved an hour after the second's; both
  reservations pending again; both recover with their original results.
- `an_unbound_reservation_with_its_scoped_run_keeps_that_run`: a round-11-shaped
  reservation (unbound, batch under the scope-named run, an epoch-less run of the key
  present too) recovers with its original counts.
- `a_pending_batch_from_an_earlier_build_recovers_into_its_run`: in a store never
  restored, an unbound reservation whose batch is under the epoch-less run recovers
  `accepted: 2, duplicates: 0`.
- `a_batch_key_reused_after_a_restore_is_its_own_batch_and_run`, extended: after the
  restore, an unbound new-epoch reservation where only the earlier epoch's epoch-less
  run exists is refused `conflict`, the archive file is byte-identical, and the
  reservation stays unbound.

Mutation checks (`m4r13/mutation-check.py`, log `m4r13/mutation-check.log`):

| Mutation | Result |
|---|---|
| U1 counts by run timestamps | killed by `receipt_counts_do_not_depend…` |
| U2 first acceptance not recorded | killed by seven receipt tests |
| U3 every unbound reservation bound to the epoch-less run (round 12) | killed by `an_unbound_reservation_with_its_scoped_run…` |
| U4 epoch-less run adopted after a restore | killed by `a_batch_key_reused_after_a_restore…` |
| U5 a lookup error treated as "scope-named run exists" | survived: no test isolates a lookup failure that the following archive write would not also hit |

Limits:

- U5 above: the refusal on a failed lookup is by inspection; the tests cannot fail the
  lookup without also failing the write.
- An archive restored from a backup older than the control store, combined with an
  earlier build's unbound reservation, is refused `conflict` rather than recovered,
  whenever the store has had a restore.
- The epoch change in the tests activates a `pending_activation` epoch through
  `Control::activate_epoch`; the full backup, restore and reopen was not re-run here.

### M4 review 13 — 9.0/10, revise (preserved)

Source: `.local/dev/runtime-mcp/m4/review-13/result.md`. All 656 hashes passed before and
after; the final inventory matched.

The reviewer confirmed the review-12 defects fixed (a pending round-11 scoped batch keeps
its run; after an actual backup, restore, activation and reopen the ambiguous new-epoch
reservation is refused `conflict` with no receipts or runs added), current-format receipt
counts stable under rollback, equal timestamps and interleaved pending batches, all five
round-13 mutations detected (U5 by a temporary-lock probe), and every earlier probe.

Material findings:

1. Older pending receipt runs (provenance without `first_accepted`) still infer
   acceptance from wall-clock ordering: a correctly bound two-record batch, a rollback,
   a duplicate batch, then recovery changed `2/0` to `0/2`.
2. Completing an older partial batch creates an incomplete acceptance record: with the
   first 57 receipts of a 200-record batch committed under older provenance, resuming
   initialised `first_accepted` with only the new receipts, so recovery reported
   `143/57` instead of `200/0`.

Minor finding:

- Collection fact counts (`run_summary.rs`, exposed by `collection.rs`) also depend on run
  timestamps: one fact linked to two runs with equal start times is a duplicate in both.
  No traffic amplification was demonstrated.

### M4 round 14 corrections (submitted for review 14)

- **Older receipt runs are never re-counted by inference.** A run written before
  acceptance records existed (its provenance has no `first_accepted`) that already holds
  receipts cannot have its counts proven. `receipts_ingest` checks the bound run
  (`run_summary::receipt_run_record`) before any effect and refuses such a batch
  `conflict` for explicit reconciliation; a lookup failure is `storage_unavailable`.
  `receipt_run_counts` no longer falls back to timestamp or run-ID ordering: an
  unrecorded run counts only when it holds nothing, and otherwise errs.
- **No partial record is ever begun.** The archive writer (`retain_chunk`) gives an
  existing unrecorded run an empty record only when the run holds no receipts. It
  refuses (and rolls back) when an unrecorded run already holds receipts, and it appends
  only to an existing record array.

Tests (`tools.rs`):

- `an_older_unrecorded_run_is_refused_not_recounted` (the reviewer's probe shape): a
  bound pending two-record batch whose run lost its record and was moved an hour later
  is refused `conflict` with the archive byte-identical. A direct archive write extending
  that run is refused, also with no effect.
- The earlier round-13 tests still pass, including the never-restored legacy recovery,
  whose run carries a record.

Mutation checks (`m4r14/mutation-check.py`, log `m4r14/mutation-check.log`):

| Mutation | Result |
|---|---|
| V1 no pre-write refusal of an older nonempty run | killed by `an_older_unrecorded_run…` |
| V2 writer begins a record on a nonempty older run | killed by `an_older_unrecorded_run…` |
| V3 record appended without the array guard | survived: equivalent, because the writer ensures the record exists before any append |

Not addressed in this round:

- The review-13 minor finding (collection fact counts using run timestamps) is not
  changed: it affects reported `new_facts`, not traffic, and is recorded for a later
  round.

Limits:

- A pending batch an earlier build wrote under the epoch-less run is now refused
  `conflict`, because such a run never carries a record. It is no longer recovered
  automatically.
- The full backup, restore and reopen procedure was not re-run by the implementer.

### M4 review 14 — 9.1/10, accept

Source: `.local/dev/runtime-mcp/m4/review-14/result.md`. All 656 hashes and the exact
inventory matched before and after.

No material findings in the reviewed M4 scope. The reviewer confirmed both review-13
defects fixed: an older bound batch after rollback and a duplicate is refused `conflict`
with the archive bytes and binding unchanged; the 57-of-200 partial batch at an actual
chunk boundary is refused extension with its 57 members unchanged; current-format batches
pass rollback, equal-timestamp, interleaved and every earlier probe; no timestamp or run-ID
ordering remains in current receipt calculation or run selection. Refusing such batches
`conflict` when historical acceptance cannot be proven is judged acceptable. What remains:
older nonempty pending batches without acceptance records need explicit reconciliation.

All 33 inherited worker/recovery probes, four SDK transport probes, the 16 schedule tests
and the identity, secret, receipt and enrichment probes pass; per-run control reservations
remain adequate alongside per-read journals; the M6 deferrals remain acceptable.

Minor findings (post-acceptance follow-ups):

1. Collection fact counts still depend on run timestamps (`run_summary.rs`): equal start
   times make both runs count a shared fact as a duplicate. No traffic effect.
2. Refusal-only receipt batches depend on archive availability: the new preflight reads
   the archive before association classification, so an entirely unassociated batch
   returns `storage_unavailable` while the archive is locked.
3. Documentation: contract §-crash replay text and the `receipt_run_counts` comment do
   not yet describe the older-run `conflict` refusal and unresolved pending batches.

### M4 post-acceptance follow-ups (review-14 minors; reviewed with M5)

These land after M4's acceptance and before M5 review 1, which covers them.

1. **Collection fact counts no longer depend on run timestamps.** A service collection
   run (one bound to a credential revision) starts with
   `provenance_json.fact_counts = {new, duplicate}`. The fact write increments it in the
   same transaction: `new` when the write created the fact, `duplicate` when it linked an
   existing one. `run_summary` reads those counts; a run without the record keeps the
   earlier derivation.
   - Test: `service_fact_counts_do_not_depend_on_run_timestamps` (equal start times, then
     a rolled-back start). Reverting to the timestamp count fails it.
2. **A refusal-only receipt batch needs no archive.** The older-run preflight now runs
   after association classification, and only when some record will be written.
   - Test: `a_refusal_only_batch_needs_no_archive`, with the archive held `EXCLUSIVE`.
     Running the preflight first fails it.
3. **Documentation.**
   - Contract §6.1 "Receipts" describes the reservation-bound run, provenance-only
     binding of earlier-build reservations, and the first-acceptance record.
   - It also describes the `conflict` refusal of unprovable older runs, which stay
     pending for explicit reconciliation.
   - The `receipt_run_counts` comment is corrected.

## M5 — Transfer and lifecycle completion

Status: **ACCEPTED at 9.1/10 by independent review 5** (2026-11-06; reviews 1–4 scored 6.0, 7.0, 7.4 and 8.5).
M4 tree together with the M4 post-acceptance follow-ups.

### M5 implementation (submitted for review 1)

- **M5.1, the B4 completion.**
  - `transfer::import_file` performs a staged, fully validated, idempotent import, and
    `transfer::validate_file` performs the same checks without a destination.
  - `repository::prune_examples` gives an examples-only preview and execute, with
    tombstones and compaction.
  - `inspect::data_generation` supplies the archive's data generation.
  - The test driver gains `export`, `import` and `prune` leaves.
  - The adversarial suite is compiled as `tests/history_transfer.rs`, with a new
    re-import check after prune. Its four tests pass:
    - valid-digest corruption (duplicate key, cardinality, orphan, forged benign
      projection) refuses with the destination unchanged, with or without existing data;
    - the export is private, no-clobber and read-only at the source, and the import is
      idempotent;
    - a wrong cloud, damaged, truncated or trailing data, and a physical SQLite file all
      refuse before any change;
    - examples-only prune: the preview writes nothing; execution erases every live copy
      and the file bytes no longer hold it; old exports are kept; re-importing the old
      export is refused.
- **M5.2–M5.4, the service.** The eight Administrator tools and the `lifecycle_execute`
  executor described in the contract (§5.1).
  - Test: `tools::tests::lifecycle_prune_export_upload_and_import_through_the_tools`.
    - The prune preview writes nothing; execution without the proof is `forbidden`; with
      the proof a job erases every live excerpt copy.
    - The export is read back in hash-bound chunks and matches its file hash.
    - That export is uploaded to a second service in owned, hash-checked chunks,
      finalized and validated, then imported through its own `import_finalize` preview.
- **Schema notes** (still undeployed schema 1): the archive table `pruned_examples`.
  `previews` already stores canonical parameters (M3).

### M5 review 1 — 6.0/10, revise (preserved)

Source: `.local/dev/runtime-mcp/m5/review-1/result.md`. All 658 manifest entries passed
before and after; the frozen source was unchanged. The reviewer's source review supports
the three M4 follow-ups (transactional fact counts, the deferred refusal-only preflight and
the corrected receipt documentation); finding 2 prevents claiming M4/M5 compatibility.

Material findings:

1. Staged validation accepts semantically invalid transfers: only the `events`
   projection is compared after rebuild; erased event identity, an example referencing an
   unowned report, an array-valued work scan ID and a forged detector projection were
   accepted (the last silently rebuilt).
2. Normal M4 service archives fail transfer validation: the metadata allowlist
   (`transfer/safety.rs`) rejects the service's `credential_revision` and `fact_counts`
   provenance; the service happy-path test seeds legacy evidence.
3. Pruned excerpts can return through ordinary retention: example insertion ignores
   `pruned_examples`; import's tombstone check examines example IDs rather than every
   inline fact copy.
4. Preview binding and keyed retries are incorrect: the data generation (run count plus
   tombstone count) misses writes inside an existing run, execution does not recheck it,
   and a retry supplies the current generation so an identical retry after a successful
   prune is refused.
5. Import validation and application race (the reader is released before the writer is
   taken; application uses `INSERT OR IGNORE`), the whole transfer is one transaction
   without bounded batches or resume checkpoints, and a post-commit error is reported as
   "before any change".
6. A failed chunk write permanently advances upload progress (`received` is committed
   before the bytes are written and synced).
7. Finalization does not establish a validated, declaration-bound upload: the session is
   finalized before content validation, a `validated:false` upload created an import
   preview, and declared `kind`, `expect_tsg` and `expect_environment` are not enforced.
8. Staging quotas, cleanup and memory bounds are not enforced: no aggregate staging
   reservation, no abandoned-upload sweeper, an expired upload cannot be aborted, and
   finalization and export read whole files into memory (transfers reach 16 GiB).
9. Transfer paths omit the service secret registry: lifecycle and upload finalization pass
   empty `SecretSet`s.
10. Required migration and service-adapter qualification are missing: no legacy-bundle
    adapter or old-SQLite migration behind `legacy_bundle`, and the adversarial suite runs
    through the store driver, not the MCP upload/job adapters. Offline restore/activation
    and M6 artifact deletion are reasonable boundaries but need a reconciled phase scope.

Minor finding:

- Upload-begin replay returns `now + TTL` instead of the persisted expiration.

### M5 round 2 corrections (submitted for review 2)

Each review-1 probe is reproduced as a deterministic test before its fix; each fix is
mutation-checked (`m5r2/mutation-check.py`, log `m5r2/mutation-check.log`).

1. **Strict semantic validation** (`transfer/semantic.rs`), on the staging copy before
   any destination exists:
   - event identities equal the identity every fact names;
   - every example is a row its event's facts derive (content addressed);
   - work descriptors decode strictly and name their work;
   - the events, event reports, detector observations and receipts equal what the
     archive's own projection derives from the staged facts.

   A forged projection refuses and nothing is silently rebuilt. Test
   `semantically_forged_transfers_are_refused_and_never_silently_rebuilt` (store), on a
   real exported archive, covers: erased identity, an unowned example report, a forged
   example excerpt, an array-valued work scan ID, and forged detector and event-report
   projections.
2. **Service provenance in strict shapes** (`transfer/safety.rs`). The run provenance
   the store's own sync and receipt paths write is accepted, typed:
   - the bound credential revision (UUID v4);
   - `fact_counts` (exactly `{new, duplicate}`);
   - `first_accepted` (unique receipt digests);
   - the progress counters.

   Export was refusing every native sync run as well as service runs. Tests:
   - `service_collection_and_receipt_archives_round_trip_through_transfer` and
     `service_enrichment_archives_round_trip_through_transfer` (airs-mcp `collection`
     target). Real service collection, receipt and enrichment runs export, validate,
     import, re-export row-for-row identically, and re-import with zero inserts.
   - `service_run_provenance_is_accepted_only_in_its_strict_shapes`.
3. **Erasure enforced on every path** (`repository.rs`):
   - a fact is stored with every tombstoned excerpt erased, keeping its received
     content's ID;
   - `examples()` never recreates a tombstoned example;
   - a receipt promoted to an event has that event's pruned excerpts erased from its
     facts;
   - import refuses a tombstoned example, and any event fact or promoted receipt fact
     carrying a pruned excerpt.

   Tests:
   - `ordinary_retention_never_resurrects_a_pruned_excerpt` (the reviewer's probe:
     repeated fact and new variant);
   - `a_promoted_receipt_never_brings_back_a_pruned_excerpt`;
   - `an_import_never_restores_a_pruned_excerpt_in_any_fact_copy` (a new variant, so
     only the erasure records can refuse it).
4. **Transactional mutation generation.** `data_generation` is SQLite's file change
   counter, read inside the caller's read or write transaction. The binding is checked:
   - for prune, inside the prune's write transaction (`prune_examples_bound`);
   - for import, inside every batch;
   - for compact and recover, immediately before they run.

   Keyed retries resolve against their original admitted binding, so the current
   generation is not part of a retry's identity. Tests:
   - `the_mutation_generation_binds_every_write_and_a_stale_prune_is_refused` (a write
     inside an existing run advances it);
   - `previews_bind_the_generation_at_execution_and_retries_keep_their_binding`: a retry
     after the prune returns its job, and a write after admission makes the job fail
     `conflict` with the archive byte-identical.

   Two authority-store assertions that encoded the defect (a retry under a moved
   generation refused) now assert the original job.
5. **Coordinated, bounded, checkpointed import** (`import_file_controlled`).
   - Staged validation, then 1,000-row batches. Each is checked (generation chain, held
     immutable content, erasure records) and applied in one write transaction under the
     archive's exclusive writer, so the reader-release race is gone.
   - A durable `importing` checkpoint is recorded after every batch, and a restarted job
     resumes from it. A final rebuild transaction follows.
   - Outcomes are `refused_before_change`, `partially_committed` (batches and rows) or
     `uncertain`; a post-commit error is never reported as "before any change".
   - Test `bounded_imports_checkpoint_resume_and_stop_on_a_concurrent_write` (3-row
     batches):
     - an import interrupted after batch 2 resumes from its checkpoint to exactly an
       uninterrupted import's archive;
     - a prune between batches stops a bound import as `partially_committed` after
       batch 1;
     - a stale binding refuses before any change, with the archive byte-identical.
6. **Upload chunk protocol** (`tools.rs`).
   - One async lock per upload. `upload_chunk_state` checks the chunk without recording
     it; the bytes are written and synced at their offset, then `upload_chunk` records
     the progress.
   - A recorded chunk's retry is accepted only when the staged bytes match. Staging is
     opened `O_NOFOLLOW` and must be a regular file.
   - Test `a_failed_chunk_write_never_advances_progress` (the reviewer's probe):
     - a failed write leaves progress 0 and the chunk can be re-sent;
     - an identical lost-response retry succeeds;
     - different bytes are `idempotency_conflict`;
     - a gap refuses.
7. **Validated, declaration-bound finalization** (`upload_finalize_admit`,
   `validate_upload`, `upload_reject`).
   - Owner and complete length are checked first. The SHA-256 is streamed, and length,
     hash, `expect_tsg`, `expect_environment` and the declared `kind`'s content are
     validated before the upload is published `finalized`.
   - An invalid upload is `aborted`, its staging removed, and it is never importable.
   - Test `an_invalid_or_misdeclared_upload_is_never_importable` (the reviewer's probe):
     invalid content is refused, and so is a valid transfer declared for another TSG,
     another environment or the wrong kind.
8. **Staging bounds.**
   - `upload_begin_within` reserves the declared length against the tenant's
     `staging_quota_bytes` in the begin transaction (`overloaded`).
   - `sweep_expired_uploads` aborts expired uploads before a new upload reserves
     staging, and their staging (verified owned regular files only) is removed. The
     owner can abort an expired upload.
   - Finalize and export hash by streaming, and validations run one at a time per
     service (`overloaded`).
   - Test `staging_is_quota_reserved_swept_and_abortable_after_expiry` covers this, and
     the minor finding: a replayed begin reports the persisted expiry.
9. **Secret registry.** Export, upload validation and import build the service's
   complete secret registry (`service_secrets`) and refuse `credential_blocked`
   without it. Test `transfers_fail_closed_without_the_secret_registry`: an export
   without one credential file fails `credential_blocked` and publishes nothing.
10. **Migration and service-adapter qualification.**
    - **`legacy_bundle`** is implemented. Finalize validates the evidence bundle
      (bounded read, normalization and privacy projection of every record), and
      `import_finalize` imports it through the operator-bound legacy import.
    - **Old-archive migration.** `airs-mcp migrate-archive --config PATH --source DIR`
      is offline maintenance:
      - the source is a recognized single owned file with no sidecars, at most 16 GiB,
        verified unchanged;
      - a private staged copy is verified, then exported by the store's own admission
        (unknown or newer schema, or another cloud, refuses);
      - a verified rollback copy of the destination is kept, then the import runs.
      - Tests: `a_development_archive_migrates_and_its_original_is_untouched` (into an
        overlapping archive; a repeat is idempotent) and
        `an_unrecognized_source_is_refused_without_changes` (newer schema, sidecar,
        missing archive).
    - **Exit-gate cases through the MCP adapters.** Test
      `adversarial_transfers_are_refused_through_the_service_adapters` runs them through
      admission, owned chunks, finalization and import execution. Duplicate key, column
      cardinality, orphan, forged projection, erased identity, unowned example,
      array-valued work scan ID, damaged digest, truncated, trailing data and a physical
      SQLite file are each refused at finalize, never importable, and create no archive.
      The valid export imports, then re-imports with zero records, and a legacy bundle
      imports through its own kind.
    - **Scope reconciliation (contract §5.1).** Offline `backup`, `restore` and
      `epoch_activate` remain offline maintenance of the stopped service, and
      `artifact_delete` is M6's (report artifacts). Each is a fixed refusal, not
      counted as an M5 deliverable.

Behaviour notes:

- **Held-record identity.** A held record's identity is its immutable content: its
  first observation time (`observed_ms`), and the first observing `run_id` of a
  profile, application or aggregate observation, are the destination's own metadata.
  Projection rows are rebuilt.
- **Why it changed.** Without this, any migration into an archive holding the same
  facts was refused.

Mutation checks (23, all killed):

| Mutation | Killed by |
|---|---|
| F1a–F1d identity, example derivation, work descriptor, projection checks off | `semantically_forged_transfers…` |
| F2a `fact_counts` not allowlisted | both round-trip tests |
| F2b typed provenance off | `service_run_provenance…` |
| F3a ingestion erasure off | `ordinary_retention_never_resurrects…` |
| F3b promotion erasure off | `a_promoted_receipt_never_brings_back…` |
| F3c import erasure check off | `an_import_never_restores…` |
| F4a generation as run + tombstone counts | `the_mutation_generation_binds…` |
| F4b prune binding unchecked | `previews_bind_the_generation…` |
| F4c retries compare the current generation | `previews_bind_the_generation…` |
| F5a batch generation chain unchecked | `bounded_imports…` |
| F5b resume checkpoint ignored | `bounded_imports…` |
| F6a lost-response retry not verified | `a_failed_chunk_write…` |
| F6b bytes not written before progress | `lifecycle_prune_export_upload_and_import…` |
| F7a invalid content accepted at finalize | `an_invalid_or_misdeclared…` |
| F7b TSG/environment declaration unenforced | `an_invalid_or_misdeclared…` |
| F8a staging quota unreserved | `staging_is_quota_reserved…` |
| F8b sweep off | `staging_is_quota_reserved…` |
| F9 secret registry not required | `transfers_fail_closed…` |
| F10a legacy bundle kind unsupported | `adversarial_transfers…` |
| F10b migration ignores source sidecars | `an_unrecognized_source…` |

Limits:

- **Uncertain commits.** The `uncertain` outcome (a failed `COMMIT`) is by inspection;
  no test injects a commit failure.
- **Partial imports.** A `partially_committed` import whose upload was consumed is
  completed by a new upload and import of the same transfer (idempotent). The consumed
  upload itself is not re-importable.
- **Compact and recover.** These check the generation immediately before running, not
  inside their own transaction; they change no data.
- **The sweep** runs when a new upload begins, not on a timer: an idle service keeps
  expired staging until the next upload or abort.
- **Legacy-bundle import** uses the existing operator-bound legacy import (its own
  batches and idempotency). Its generation binding is checked immediately before it, not
  inside its transactions.
- **Migration** runs offline only; it was tested on synthetic archives, not a real
  development archive.
- **Sandbox loopback.** The service-adapter tests use the in-process handlers, not the
  HTTP listener (as in M4's handler tests).

### M5 review 2 — 7.0/10, revise (preserved)

Source: `.local/dev/runtime-mcp/m5/review-2/result.md` (full review
`m5/review-2/scratch/review.md`). All 661 manifest entries passed before and after.

The reviewer reran every review-1 probe (17 probes; four reproduce remaining defects),
confirmed exact 256 KiB records pass the upload/finalize adapters and wrong
TSG/environment/kind refuse, judged the offline-maintenance, M6 artifact-deletion and
narrower prune-scope boundaries reasonable once consistently incorporated, judged ignoring
destination-local observation timestamps and first run IDs during deduplication
reasonable, and found the three M4 follow-ups still supported.

Material findings:

1. Compaction reuses preview generations: SQLite's file change counter resets when
   compaction publishes its `VACUUM INTO` copy, so a stale preview can become valid again.
2. Receipt-only excerpts can return after pruning: tombstones are derived only from
   `examples`, but promoted receipt facts can carry excerpts without example rows.
3. Import discovers destination conflicts after earlier batches commit (held-key and
   tombstone checks are per batch); the missing-archive generation exemption keeps a
   concurrent-creation race.
4. Crash recovery does not reconcile committed, uncheckpointed batches (progress is
   recorded in the control store after the archive commit, without an archive operation
   ID); first-batch failures can report `refused_before_change`.
5. Import ignores cancellation at checkpoints.
6. Consumed uploads escape quota and cleanup permanently (marked `imported` at admission;
   never removed).
7. Semantic validation still grows memory with the whole transfer.
8. Migration rejects the actual M4 predecessor schema (no `pruned_examples`); tests use
   current-schema sources.
9. Online recovery cannot reach a hot journal: preview, admission and execution require
   ordinary archive reads, which reject journal sidecars.

Minor findings: preview counts and generation come from separate snapshots; §5.1's
deferrals need reconciling with the older online-maintenance promise, the unchanged M5
plan row and "removed after 24 h" versus begin-triggered sweeping, and should state that
`control-*` backup/restore covers the control store.

### M5 round 3 corrections (submitted for review 3)

1. **A generation never repeats across compaction or recovery.**
   - The generation is the archive's incarnation (new table `archive_generation`, a
     random value) in the high 32 bits and SQLite's file change counter in the low 32.
   - Compaction gives its `VACUUM INTO` copy a fresh incarnation before verifying and
     publishing it. A recovery that rolls a journal back does the same; a healthy archive
     that recovery merely verifies stays byte for byte unchanged.
   - Tests: `database::a_generation_never_repeats_across_compaction` (three compactions,
     writes well past the old counter, no generation repeats) and
     `tools::a_preview_from_before_a_compaction_never_validates_again` (service level:
     after a compaction and enough writes to bring the counter back to the preview's
     value, executing the stale prune preview is refused).
2. **Every stripped fact copy is tombstoned.** Prune's erasure (`repository::erase_events`)
   records a tombstone for every excerpt-bearing snippet of every event fact and every
   promoted receipt fact (under the event the copy belongs to, which is what storage and
   promotion check), not only for `examples` rows, before anything is stripped.
   - Test: `a_receipt_only_excerpt_is_tombstoned_and_never_resurrects` (the reviewer's
     probe through the real `erase_events`; then the same receipt, a new variant, another
     receipt and a repeated event fact bring nothing back).
3. **Whole-transfer prevalidation, and creation is not a race.**
   - Before the first batch, every staged row's held key and erasure records are checked
     against one destination snapshot (a write transaction under the writer gate, rolled
     back); its generation binds the first batch, and every later batch must find the
     generation the previous one left.
   - An archive the import saw missing must still be empty (`pristine`) when the first
     batch creates it.
   - Batches are positioned within their table (`LIMIT/OFFSET` in the staging store)
     instead of rescanning earlier rows in Rust.
   - Tests: `a_late_destination_conflict_refuses_before_any_change` (one row per batch,
     the conflict beyond the first batch; the archive is byte-identical) and
     `a_concurrently_created_archive_refuses_the_first_batch`.
4. **The commit-to-checkpoint window is reconciled (C4).**
   - Every batch, and the final rebuild, writes its progress to the archive's own
     `import_operations` journal (operation ID = the job ID; transfer SHA-256, batches,
     rows, inserted, rebuilt, the generation the transaction started from) in the same
     transaction.
   - The journal is read before anything else. On a restart it, not the control
     checkpoint, decides where the import resumes (the chain continues from its
     generation plus the one commit), and any refusal after committed batches is
     `partially_committed`, never `refused_before_change`.
   - Test: `committed_but_unchecked_batches_are_reconciled_from_the_archive_journal`.
     Batch 2 commits but its control checkpoint is never written (the closest durable
     injection: the callback that persists the checkpoint fails after the archive
     commit). A restarted job refused at validation reports 2 committed batches. Resumed
     from its stale batch-1 checkpoint, the job completes exactly once; a restart with no
     control checkpoint at all reconciles too. A real process death is also exercised:
     `a_real_process_death_between_commit_and_checkpoint_is_reconciled` runs the import in
     a child test process that exits (status 86) inside the per-batch callback. That is
     after batch 2 committed in the archive and before its control checkpoint was
     persisted. Restarts from the stale batch-1 checkpoint, and with no checkpoint,
     complete exactly once with the uninterrupted import's counts.
5. **Cancellation is honoured at checkpoints.**
   - The executor's checkpoint callback turns an observed cancellation into a stop
     before the next batch or the rebuild.
   - A new outcome, `Outcome::CanceledWith`, lets the worker end the job `canceled` with
     the partial-commit details as its result. Other executors are unchanged.
   - Tests: `tools::an_import_canceled_at_a_checkpoint_stops_and_reports_its_batches`
     (canceled while it runs: canceled after one committed batch, the journal shows no
     rebuild) and `worker::a_cancellation_after_durable_effects_keeps_its_details`.
6. **Consumed uploads keep their quota until their staging is removed.**
   - Imported and aborted uploads reserve their bytes until a verified removal (new
     upload state `removed`).
   - The staging sweep (`tools::sweep_staging`, on every upload begin and now on every
     worker pass) removes the staging of aborted uploads and of uploads whose consuming
     job has ended, whatever its outcome, and only then releases the quota. A restart
     reconciles whatever an earlier sweep left.
   - Test: `a_consumed_upload_keeps_its_quota_until_its_staging_is_removed` (the
     reviewer's probe: while the job is queued, the file is kept and reserved even past
     expiry; once the job failed, the sweep removes it and only then releases it; the
     owner cannot abort it).
7. **Semantic validation is bounded in memory.**
   - The staging store is disk-backed with a bounded cache and `temp_store=FILE`, under
     the import's deadline and cancellation handler.
   - Derived identities and examples go to staging tables, compared in SQL. Projections
     are copied in the staging store, rebuilt in keyset pages, and compared with `EXCEPT`
     both ways; no whole-transfer Rust map, set, dump or ID list remains.
   - Test (opt-in): `large_transfer_validation_stays_under_a_memory_ceiling`
     (`AIRS_LARGE_TRANSFER_EVENTS`, release, run alone) measures peak RSS growth during
     validation against a 128 MiB ceiling. Measured (release, tmpfs): 20,000 events,
     260,042 rows, a 92.9 MB transfer, 13.6 MiB peak RSS growth.
8. **The accepted M4 schema is recognized and upgraded.**
   - The archive schema is version 2. The frozen accepted-M4 schema
     (`schema_v1.sql`, copied from the accepted M4 snapshot) is version 1: read as is, and
     upgraded in place by the first writer (one exclusive transaction adds
     `pruned_examples`, `archive_generation` and `import_operations`).
   - `migrate-archive` therefore accepts an M4 source; unknown and newer schemas refuse
     unchanged.
   - Tests: `database::an_accepted_m4_archive_is_read_and_upgraded_in_place` (the
     fixture is asserted signature-identical to `SCHEMA_V1`; an unknown version-1
     variant refuses unchanged) and `migrate::an_accepted_m4_archive_migrates`.
9. **Online recovery reaches a hot journal.**
   - `recover` previews, admission and execution are bound to
     `recovery_identity`: the archive's and journal's file identities and headers, read
     without admitting either to SQLite, with the journal admitted and WAL/SHM refused as
     `recover` itself checks.
   - Execution then runs the existing exclusive verified-copy recovery.
   - Test: `tools::a_hot_journal_is_recovered_through_the_service`. A real hot journal is
     made by a spilled, uncommitted transaction; the ordinary preview refuses it; recover
     previews, executes and rolls it back to exactly the pre-crash contents.

Mutation checks (each fix disabled alone; scripts and logs under `m5r3/`):

| Round 3 (`mutation-check.py`, 14) | Killed by |
| --- | --- |
| F1a compaction keeps the old incarnation | `a_generation_never_repeats_across_compaction`, `a_preview_from_before_a_compaction_never_validates_again` |
| F1b generation ignores the incarnation | the same two, and the round-2 generation test |
| F2 fact/receipt copies not tombstoned | `a_receipt_only_excerpt_is_tombstoned_and_never_resurrects` |
| F3a no whole-transfer prevalidation | `a_late_destination_conflict_refuses_before_any_change` |
| F3b created archive not required empty | `a_concurrently_created_archive_refuses_the_first_batch` |
| F4 archive journal not consulted | `committed_but_unchecked_batches_are_reconciled…`, `bounded_imports…` |
| F5a lifecycle ignores cancellation | `an_import_canceled_at_a_checkpoint_stops_and_reports_its_batches` |
| F5b worker drops the canceled details | that test and `a_cancellation_after_durable_effects_keeps_its_details` |
| F6a consumed uploads not reserved | `a_consumed_upload_keeps_its_quota_until_its_staging_is_removed` |
| F6b ended jobs' staging never removed | the same test |
| F7 projection comparison disabled | `semantically_forged_transfers_are_refused_and_never_silently_rebuilt` |
| F8a version 1 not upgraded | `an_accepted_m4_archive_is_read_and_upgraded_in_place` |
| F8b version 1 not readable | the same test |
| F9 recover bound to the ordinary generation | `a_hot_journal_is_recovered_through_the_service` |

All 23 round-2 mutations (`mutation-check-round2.py`, mapped to this round's code where
it was rewritten: the import erasure check, the batch chain, resume and the sweep) are
still killed. The batch-chain mutation first survived (its round-2 test's interleaved
write was a prune, which the per-batch erasure check also refuses); it is killed by the
added `an_unrelated_write_between_batches_stops_the_import`.

Minor findings:

- A prune preview's counts and its generation come from one read transaction
  (`prune_examples` returns the generation its reader saw; the tool binds that).
- Contract §5.1:
  - records the scope reconciliation (restore and epoch activation offline,
    `control-*` covering the control store, artifact deletion in M6, M5.4 online =
    compact and recover), and the M5 plan row is amended to point at it;
  - corrects §9.1's "the same operations over MCP";
  - states the 24 h expiry and the begin-and-worker sweep.

Behaviour changes:

- The archive schema version is 2 (version 1 is upgraded on first write).
- An import without an operation ID gets a fresh one.
- A restarted import resumes from the archive journal; with neither journal nor control
  checkpoint it starts fresh. A control checkpoint without a journal refuses.
- Upload states gain `removed` (control schema).

Limits:

- C4 is exercised by failing the checkpoint callback after the archive commit and, since
  this round was submitted, by a real process death:
  `transfer::review_tests::a_real_process_death_between_commit_and_checkpoint_is_reconciled`
  runs the import in a child test process that exits between an archive batch commit and
  its control checkpoint.
- Between batches the destination is visibly incomplete (recorded as such in
  `import_operations`; readers did not consult it until round 4).
- The journal reconciliation assumes one counter step per committed transaction (it
  holds in rollback-journal mode, and the reconciliation test depends on it).
- The incarnation is random: a repeat across incarnations has probability 2^-31 per
  renewal.
- An import's prevalidation opens a writer, which upgrades a version-1 destination
  before refusing.

### M5 review 3 — 7.4/10, revise (preserved)

Source: `.local/dev/runtime-mcp/m5/review-3/result.md`. All 662 manifest entries passed
before and after.

The reviewer confirmed: the consumed-upload quota and cleanup fix; the
checkpoint-cancellation fix; the archive-side import journal; whole-transfer validation;
the accepted-M4 archive schema being recognised; the semantic-validation memory fix
(260,001 rows, 82 MB, peak 50.7 MiB RSS under a 512 MiB ceiling); and the receipt-only
erasure and promotion tests. It found the three M4 follow-ups still sound, and the §5.1
and plan-row scope reconciliation acceptable.

Material findings:

1. An empty receipt ID bypasses projection validation (`semantic.rs` keyset cursor
   starts at `""` and selects only greater IDs): a forged promoted receipt without
   supporting facts passed the real chunk and finalize handlers.
2. Accepted-M4 control stores cannot reopen: `control.sql` changed the upload-state
   constraint while the schema version stayed 1, so `Control::open` on an M4 store
   returns `maintenance_required`. This is an M4 regression.
3. M4 archive handling: the import opens a writer (committing the schema upgrade) before
   destination validation, so a refusal can claim `refused_before_change` after changing
   the archive, and the upgrade invalidates an M4 preview's generation; hot-journal
   recovery renews the incarnation on a version-1 archive that lacks that table.
4. Late cancellation discards committed-import results: only `CanceledWith` keeps its
   result in the worker.
5. Interrupted imports expose unfinished projections without an effective incomplete
   marker (rebuild happens last; readers do not consult `import_operations`).
6. Prune fails for ordinary large selections: every selected event ID goes into one SQL
   parameter (20,000 IDs exceed the 1 MiB value limit); prune also materialises whole
   fact sets.
7. The shared adversarial suite is not run against the service adapters (a separate
   service test copies selected shapes).

Minor findings: lifecycle reports tombstones from the example-row count; the ledger
still says C4 was not tested with child-process death; the incarnation is a random
31-bit value (probabilistic, not "never repeats").

### M5 round 4 corrections (submitted for review 4)

1. **Every key is validated; receipts carry their identity.** Semantic keyset paging
   starts before every key (a NULL cursor), so an empty ID is visited. A receipt's ID
   must be its origin key's digest (never empty), and at least one fact of its own must
   support it; a forged promoted receipt is refused whatever its ID, through the store
   and through the service adapters (the shared cases include the empty-ID and
   nonempty-ID forgeries).
   - Tests: `a_forged_receipt_is_refused_whatever_its_id`,
     `keyset_paging_visits_every_key_including_the_empty_one`.
2. **Accepted-M4 control stores migrate.** The control schema is version 2; version 1 is
   frozen in `control_v1.sql`. `control::migrate_file` migrates a version-1 store whose
   structure is exactly version 1's, on a private copy (open: `verify_copy`; restore:
   the staged copy), by widening the one changed constraint through SQLite's
   `writable_schema` procedure, then checks integrity and the exact version-2
   fingerprint. `Control::open` publishes the migrated copy and preserves the original
   under `preserved-…/`. A version-1 backup set restores. Any other structure or version
   is refused unchanged.
   - Test: `an_accepted_m4_control_store_migrates_on_open_and_restore`. It covers jobs
     and idempotency surviving, the original preserved byte for byte, the version-1
     restore, and an unknown version-1 variant refused unchanged.
3. **Accepted-M4 archives: validation never changes them; the upgrade is backed up.**
   - Import prevalidation opens the destination with `Writer::existing_unupgraded`, so a
     refusal leaves a version-1 archive byte for byte unchanged and still version 1.
   - Every other writer upgrades a version-1 archive only after a verified private copy
     of it (`.pre-upgrade-v1.<id>.sqlite3`, size and SHA-256 in a durable manifest).
   - The writer records the generation it upgraded from, and `Writer::bound_to` admits
     exactly that transition. So a preview or import bound to the version-1 generation
     still holds across the writer's own upgrade (prune, import batches); any other
     change refuses.
   - A hot journal on a version-1 archive is recovered (verified copy first, raw copies
     kept), and the recovered archive becomes version 2 in the same transaction.
   - Tests: `an_import_into_an_m4_archive_refuses_unchanged_or_upgrades_then_imports`,
     `the_m4_upgrade_keeps_a_verified_pre_upgrade_copy_and_is_never_implicit_in_validation`,
     `a_hot_journal_on_an_m4_archive_recovers_to_version_2` (a real child-process hot
     journal).
4. **A late cancellation keeps the committed result.** When the job is
   `cancel_requested` after the executor's last checkpoint, the worker ends it
   `canceled` with the executor's result (success, partial or failure), in one
   transaction.
   - Test: `a_late_cancellation_keeps_the_committed_result_for_every_outcome`.
5. **Interrupted imports are consistent and visible.** Every import batch rebuilds, in
   its own transaction, the projections of the events and receipts it added facts to,
   so every committed batch leaves projections consistent with all held facts. The
   archive status reports `unfinished_imports` (from `import_operations`), and
   `history_coverage` refuses `maintenance_required` while an import is unfinished.
   - Tests: `an_interrupted_merge_keeps_projections_consistent_and_reports_the_import_unfinished`
     (a conflicting fact for a held event, stopped before publication; projections equal
     a completed import's), `coverage_refuses_while_an_import_is_unfinished`.
6. **Prune scales.** The selection is SQL (a subquery for the preview; a temporary
   selection table under the writer), never one bound ID list. Tombstoning, stripping
   and rebuild walk bounded keyset pages.
   - Test: `a_prune_of_twenty_thousand_events_stays_within_sqlite_limits` (20,000
     excerpt-bearing events under the 1 MiB value limit; preview and execution agree;
     every copy erased).
   - Receipt-only excerpts share the same erasure path:
     `a_receipt_only_excerpt_is_tombstoned_and_never_resurrects`.
7. **One adversarial suite, both adapters.** `tests/support/transfer_cases.rs` builds
   the exit-gate cases from a valid export: duplicate key, cardinality, orphan fact run,
   forged projection, erased identity, unowned example, array work scan ID, forged
   receipts (empty and nonempty ID), wrong cloud, damaged, truncated and trailing. Both
   adapters run exactly these, into empty and populated destinations:
   - the store driver: `shared_adversarial_cases_refuse_through_the_driver`;
   - the service adapters (admission, chunks, finalize, import):
     `adversarial_transfers_are_refused_through_the_service_adapters`, plus the physical
     SQLite upload.

   Service coverage of the rest of the gate: `lifecycle_prune_export_upload_and_import_through_the_tools`
   (prune erases every copy, checked on raw archive bytes; the published export is now
   asserted owner-only regular; import and idempotent re-import) and
   `an_import_canceled_at_a_checkpoint_stops_and_reports_its_batches` (interruption).

Minor findings:

- The lifecycle result reports the tombstones the erasure recorded.
- The stale C4 limitation is corrected.
- The incarnation is a durable monotonic counter (strictly increasing within a lineage;
  a new lineage starts at a random value below 2^30, stated as such).
  - Test: `incarnations_strictly_increase_and_never_wrap`.

Mutation checks (`m5r4/mutation-check.py`, log `m5r4/mutation-check.log`):

| Mutation | Result |
|---|---|
| A1 keyset paging starts at `""` | killed by `keyset_paging_visits_every_key…` |
| A2 no receipt identity check | survived in round 4, wrongly classified as equivalent (review 4): an `origin_key` change passes projection validation. Killed after round 5 by `transfer::review_tests::a_receipt_with_a_changed_origin_key_is_refused_by_its_identity` |
| A1 and A2 together (the review-3 code) | killed by `a_forged_receipt_is_refused…` |
| B1 version-1 control store not migrated | killed by `an_accepted_m4_control_store…` |
| B2 migrated copy not published | killed by `an_accepted_m4_control_store…` |
| C1 prevalidation upgrades the M4 archive | killed by `an_import_into_an_m4_archive…` |
| C2 binding ignores the writer's own upgrade | killed by two tests |
| C3 no pre-upgrade backup | killed by `the_m4_upgrade_keeps…` |
| C4 v1 hot-journal recovery renews a missing incarnation | killed by `a_hot_journal_on_an_m4_archive…` |
| D1 late cancellation keeps only `CanceledWith` | killed by `a_late_cancellation_keeps…` |
| E1 no per-batch projection rebuild | killed by `an_interrupted_merge…` |
| E2 coverage ignores unfinished imports | killed by `coverage_refuses_while…` |
| G1 incarnation renewal not monotonic | killed by `incarnations_strictly_increase…` |
| H1 lifecycle reports example rows as tombstones | killed (after round 4) by `tools::tests::a_prune_reports_its_recorded_tombstones_not_its_example_rows`, whose fixture removes the example rows so fact copies alone yield tombstones |

Limits:

- `writable_schema` is SQLite's documented constraint-change procedure; the migration
  verifies the result rather than trusting the procedure.
- The pre-upgrade copies are not removed automatically (an operator removes them).
- (Resolved after round 4: a service test now separates tombstones from example rows, killing H1.)

### M5 review 4 — 8.5/10, revise (preserved)

Source: `.local/dev/runtime-mcp/m5/review-4/result.md`. All 664 manifest entries passed
before and after.

The reviewer confirmed every review-3 finding fixed or (finding 5) partly fixed: the
empty-ID bypass refused through the real handlers; accepted-M4 control stores reopen;
the M4 archive paths corrected; late cancellation keeps its result; prune erases 20,000
events and 80,000 example rows; the shared cases refuse through both adapters. It also
confirmed the minors and every earlier fix, the three M4 follow-ups, all 13 authority and
26 control tests, the scope reconciliation, and validation of 260,001 rows under a
512 MiB ceiling (peak 51 MiB).

Material findings:

1. A replacement import leaves coverage blocked by the previous import: journal completion
   is updated only for the current operation ID, so a canceled or failed import's
   `rebuilt=0` row survives a successful re-import of the same transfer and
   `history_coverage` refuses indefinitely.
2. The incomplete-import coverage guard races the coverage read: the check and the
   coverage are computed in two separate archive readers.

Minor findings: MCP status drops `unfinished_imports`; mutation A2 was wrongly classified
as equivalent (an `origin_key` change passes projection validation, and only the identity
check refuses it); the final rebuild collects complete receipt and event ID lists.

### M5 round 5 corrections (submitted for review 5)

1. **A replacement import retires the earlier unfinished one.** When an import of a
   transfer publishes (its final rebuild commits), the same archive transaction marks
   every earlier unfinished import of the *same* transfer (same SHA-256) finished. Their
   journal rows are kept for audit with their batch, row and insert counts; only `rebuilt`
   and `updated_ms` change. Unfinished imports of other transfers stay unfinished.
   - Test: `transfer::review_tests::a_replacement_import_of_the_same_transfer_retires_the_unfinished_one`
     (a stopped import, then a fresh job importing the same transfer in full; coverage
     refuses before and reads after; both journal rows remain). Removing the supersede step
     fails it.
2. **Coverage and its unfinished-import guard share one read transaction.**
   `inspect::coverage_if_published` performs the unfinished check and the whole coverage
   read through one reader, inside that reader's own `BEGIN DEFERRED` transaction
   (committed by `finish`). No import can commit between them. `history_coverage` uses it
   and refuses `maintenance_required` while an import is unfinished. The interleaving is
   excluded by construction (one transaction), so there is no timing test; the replacement
   test exercises both outcomes of the function.

Minors:

- `history_status` returns `unfinished_imports` (added to its result schema as an optional
  non-negative integer); `coverage_refuses_while_an_import_is_unfinished` asserts it.
- Mutation A2 is reclassified (see the round-4 table) and pinned by
  `a_receipt_with_a_changed_origin_key_is_refused_by_its_identity`, a real
  `ingest_client_receipts` receipt whose `origin_key` is changed in the export.
- The final rebuild walks staged receipt and event IDs in keyset pages of 1,000 from a
  NULL start, instead of collecting complete lists.

Limits: the paged rebuild is not separately qualified under a memory ceiling (staging
validation is, at 260,001 rows); no deterministic interleaving test exists for finding 2
beyond its single-transaction construction.

### M5 review 5 — 9.1/10, accept

Source: `.local/dev/runtime-mcp/m5/review-5/result.md`. All 664 manifest entries passed
before and after.

No material findings. The reviewer confirmed every review-4 finding fixed:
- replacement publication retires same-transfer unfinished rows atomically, keeps audit
  counts, and preserves a different transfer's unfinished row;
- the coverage guard and query share one reader transaction (a deterministic SQLite probe
  confirmed an intervening writer cannot commit inside it);
- status exposes `unfinished_imports`;
- A2 is correctly reclassified;
- the final rebuild pages IDs from a NULL start.

It found every earlier correction still supported: staged semantic validation,
whole-transfer prevalidation, the archive journal, cancellation, erasure across all copies,
the upload protocol, authority, the accepted-M4 archive and control migrations, and the
three M4 follow-ups. It found no new M2–M4 regression and judged the reconciled scope
acceptable.

Minor findings (post-acceptance follow-ups):

1. Retain complete recovery and interleaving regressions: an MCP-level
   cancellation → fresh upload/job → successful import → usable coverage test, a
   coordinated writer between the coverage guard and query, and an unfinished
   different-transfer negative control.
2. Qualify a complete import, including the final rebuild, under a declared memory
   ceiling (only staging validation has memory-ceiling evidence).

### M5 post-acceptance follow-ups (review-5 minors)

2. **Full-import memory qualification.** The opt-in
   `transfer::review_tests::large_transfer_import_stays_under_a_memory_ceiling` imports a
   20,000-event transfer into a fresh archive, covering staging, validation, every batch
   and the paged final rebuild. The transfer is 260,042 records and 92.9 MB, applied in
   261 batches. Peak RSS grew 13.4 MiB, against a fixed 128 MiB ceiling; validation alone
   grew 11.2 MiB. This was a release build on local disk. Run it with
   `AIRS_LARGE_TRANSFER_EVENTS=<n> cargo test --release -p airs-history-store --lib
   large_transfer_ -- --ignored --test-threads=1`.

1. **Recovery and interleaving regressions.**
   - `tools::tests::a_fresh_import_job_retires_a_canceled_one_through_the_tools`, through
     the real upload, finalize and import tools. A destination holds a canceled import's
     unfinished journal row for the transfer, plus an unfinished import of a different
     transfer (the negative control); status reports 2 and coverage refuses. A fresh
     upload and job then import the same transfer: status reports 1, the retired row is
     kept, and coverage still refuses until the other transfer's import is resolved.
   - `inspect::tests::no_writer_commits_between_the_unfinished_check_and_the_coverage_read`,
     the coordinated writer. While the coverage reader is open, a writer's journal commit
     is refused (busy); once the reader finishes, the same write commits.

## M6 — Historical daily reports and retrieval

Status: **ACCEPTED, 9.0/10 (review 7; review 6 also 9.0) under the owner-set 9.0 threshold** (2026-11-06; previously 9.1). Reviews 1–5 scored 7.8, 8.5, 8.9, 8.8 and 8.9. Round 8 (recovery-field verification, the review-7 material finding) is in the tree; review 8 was stopped unfinished when the threshold changed, and no score is claimed for it.
Written on earlier M4 rounds, completed in a second pass (full-cohort summary, HTML
filters under a hashed CSP, golden tests, opt-in 1M-scan qualification), and carried onto
the accepted M4 tree with M5 by cherry-picking the M6 commits only. The contract's §6.2
status note describes the behaviour.

Carrying M6 onto accepted M4 changed one behaviour: daily report and enrichment
submissions now write their slot row and the job's `scheduled_slot` in the submission
transaction (the M4 review-9 pattern), pinned by
`daily_store::a_daily_slot_association_commits_with_its_job`.

M6's report tables (`reports`, `report_versions`, `assessments` and their indexes) make
the control schema version 3. `control::migrate_file` takes a version-2 store (accepted
M4 + M5; frozen `control_v2.sql`) to version 3 by creating only those new, empty objects,
and a version-1 store through both steps in one exclusive transaction; the result must
fingerprint exactly as a fresh version-3 store. Pinned by
`control::tests::a_version_2_control_store_migrates_to_the_m6_schema` and the
accepted-M4 migration test.

### M6 implementation

- **M6.1 Report input and cohort queries.**
  - `airs_history_store::report::report_evidence`: period events by cutoff, rebuilt with
    `cached_event`. Profiles and applications are taken as of the period end. Coverage
    per category is conservative; unknown-time events become gaps; truncation is
    stated.
  - `airs_history_store::query::cohort` backs `history_query`.
- **M6.2 Daily windows and revisions.** `daily_store.rs`: Chicago midnight, DST days,
  serial catch-up, revision opportunities and their cutoffs, daily enrichment.
  `schedule_set_kind` and `schedule_disable` now work for every kind.
- **M6.3 Bundles and retrieval.** `reports.rs`: staged, fenced, immutable publication,
  the latest pointer and C5–C7 recovery. The tools are `reports_create`, `reports_list`,
  `reports_get`, `artifacts_read` and `artifacts_download`.
- **M6.4 Assessments and completion events.** `assessments_submit` is bound to the
  review hash and never changes evidence, and a `report_published` event is recorded
  with each publication.
- **Control schema (undeployed schema 1):** new tables `reports`, `report_versions` and
  `assessments`.

Tests:

- `tests/collection.rs` (loopback fixture):
  - `reports_render_offline_into_immutable_versions_and_deduplicate`: zero AIRS
    requests, 0600 files matching the manifest, the frozen public manifest, no new
    version for identical evidence, and a new version with a predecessor after changed
    evidence, with the old version unchanged;
  - `report_recovery_reattaches_or_quarantines_by_fence`: C6 reattach, the fenced case
    quarantined, the stage discarded;
  - `cohort_queries_count_the_whole_range`;
  - `the_daily_schedule_publishes_a_daily_report`.
- `daily_store` unit tests:
  - `daily_reports_cover_the_previous_local_day_across_dst`: 23, 25 and 24 h;
  - `missed_days_catch_up_serially`;
  - `revisions_coalesce_to_the_latest_due_opportunity`: the +24 cutoff is kept when
    late;
  - `daily_enrichment_runs_only_when_enabled_after_the_report`.
- `tools` unit tests:
  - `reports_and_artifacts_are_retrievable_bounded_and_hash_bound`: pages and chunks
    reassemble, another principal's cursor is refused, another tenant gets `not_found`,
    and a tampered file is refused;
  - `assessments_are_attributed_and_never_change_evidence`;
  - `manual_reports_are_quota_bound_and_enrichment_resolves_reports`: the hourly quota
    with its retry hint, a replay not re-charged, and non-hour ranges refused.
- Service tests now list `reports_list` as callable for the reader.

### M6 completion (closing the recorded M6.1 deferrals)

The first M6 build deferred the historical page's filters and hashed-script CSP, the
golden tests and the 1M-scan qualification. Those deferrals are closed here, and an
as-of defect in posture selection is fixed.

- **Full-cohort summary** (`airs_history_store::report`). It is read in the same
  coherent snapshot as the detail, with the evidence cutoff applied.
  - Event counts by America/Chicago report date, application, verdict and action.
    The date is computed in SQL from compiled offset segments,
    `schedule::report_offset_segments`.
  - Detector-positive event counts by detector.
  - Budget: 50,000 cells. Fallback order: detector, then date, then application. Each
    disabled filter is stated as a gap and bound into the digest.
- **Posture as of the period end by observation link.** Previously the latest
  `profile_observations.observed_ms` was used, so A, B, A reobservation reported B.
  Selection now goes through `profile_runs`/`application_runs`, at or before the
  period end.
- **Profile timeline:** the configuration at the period start, then each observed
  difference, capped at 1,000 spans.
- **Historical page** (`airs-mcp/src/report_html.rs`). It is service-only: the CLI's
  script-free Signal Brief is unchanged, so no CLI parity difference arises here.
  - Deterministic output.
  - CSP `default-src 'none'`, with one hashed stylesheet and one hashed script, and
    Trusted Types required.
  - Date, application, detector, verdict and action filters recompute cards, tables
    and per-date meters from the summary.
  - Capped detail table.
  - Profile/service matrix: the Signal Brief's own (`html::profile_matrix`), with
    explicit enablement only.
  - Timeline, coverage and gaps.
  - The no-script and print views show the base scope.
  - The 32 MiB fallback drops the detail table; a page still over it is refused.

Tests added:

- History store (`report_tests.rs`):
  - `full_cohort_summary_is_exact_beyond_the_detail_bound`: 12,000 events. The
    capped detail holds no malicious event, while the summary counts 2,000 malicious,
    2,000 blocked and 2,000 detector-positive. An unknown-time event is excluded.
  - `the_summary_budget_disables_filters_in_order_at_its_exact_boundaries`: budgets
    18, 17, 11 and 5 on an 18-cell cohort.
  - `the_production_cell_budget_is_exact`: 50,000 cells fit; 50,001 disables the
    detector filter.
  - `summary_dates_follow_chicago_across_dst`: the 25-hour 1 November day, and the
    split at local midnight across 8 March.
  - `late_data_enters_only_revisions_whose_cutoff_covers_it`.
  - `profiles_are_taken_as_of_the_period_end_by_observation_link`: A, B, A, a future
    revision, and the timeline.
  - The opt-in `qualify_30_and_90_day_reports_on_a_million_scans`.
- Core: `offset_segments_place_every_hour_of_dst_days_on_its_local_date`.
- Service page (`report_html_tests.rs`):
  - golden snapshot `crates/airs-mcp/tests/golden/report.html`, plus determinism.
    Regenerate deliberately with `AIRS_UPDATE_GOLDEN=1`;
  - the CSP admits exactly the page's style and script;
  - HTML, attribute and script-block injection fixtures;
  - 10,000 and 10,001 detail rows;
  - the byte ceiling at exactly its length, one under it (detail dropped), and refusal;
  - disabled-filter rendering;
  - the no-script and print views;
  - the filter script's arithmetic, executed under Node against the server totals
    (skipped and stated when Node is absent).
- Golden coverage: `the_report_input_matches_its_golden_coverage_snapshot`, run on a
  seeded archive against `crates/airs-history-store/tests/golden/report-input.json`.

1M-scan qualification (run with `cargo test --release -p airs-history-store --lib
qualify_ -- --ignored --nocapture`, local tmpfs):

- The run seeded 1,000,000 distinct synthetic scans over 90 Chicago days, with 40
  applications and 10% detector-positive, through the archive's record path in
  1,000-event transactions. Seeding took 79 s.
- 30-day input: 333,334 events, 2,400 + 720 cells, no disabled filter, 1.8 s.
- 90-day input: 1,000,000 events, 7,280 + 2,178 cells, no disabled filter, 5.3 s.
- Both cohorts were exact, the detail was capped at 10,000, and both reads finished
  well inside the 60 s report read budget. They do not fit the 2 s ordinary-read
  budget, which is why reports have their own.
- Not measured: peak memory and production I/O. Those remain M8 evidence on
  provisioned storage.

Exit-gate inventory:

| Item | Tests |
|---|---|
| Golden report and coverage | the two golden snapshots |
| DST | `daily_reports_cover_the_previous_local_day_across_dst`, `summary_dates_follow_chicago_across_dst`, offset segments |
| Missed ticks | `missed_days_catch_up_serially`, `revisions_coalesce_to_the_latest_due_opportunity` |
| Late data | `late_data_enters_only_revisions_whose_cutoff_covers_it`; changed evidence makes a new version with a predecessor |
| Publication crashes | `report_recovery_reattaches_or_quarantines_by_fence` (C5–C7) |
| Cardinality thresholds | the budget tests; 10,000 detail rows; the 32 MiB boundary |
| Unauthorized retrieval | `reports_and_artifacts_are_retrievable_bounded_and_hash_bound` |
| Zero Reports calls | `reports_render_offline_into_immutable_versions_and_deduplicate` |

Remaining explicit limits:

- The 32 MiB boundary is tested by injecting the ceiling. With the 10,000-row and
  50,000-cell bounds, a real page stays far below it.
- The qualification uses synthetic metadata-only scans on local tmpfs, seeded through
  the archive's own record path. Production storage, I/O and memory are M8 evidence.
- Seeding at scale found that one write transaction of about 10,000 events into an
  archive of about 400,000 hits SQLite's 256 MiB heap limit (`cache_spill=OFF` keeps
  every dirty page in memory). Collection writes bounded pages per transaction, so
  this is recorded for M8 sizing rather than changed here.
- `summary.md` is the stored summary, not a fourth bundle file.
- `tzdb_version` is reported as `jiff-bundled`.
- Chunk reauthorization is per request, through the existing dispatch, with no
  separate chunk token.
- When M7's CLI retrieves `report.html`, its `--mcp` documentation should name this
  page as the service's, distinct from the local script-free brief.

### M6 artifact deletion (moved from M5)

The §5.1 scope reconciliation moved `artifact_delete` (report-artifact deletion) from M5
to M6; it was not implemented when M6 was carried onto the accepted-M5 tree, and is now
delivered (contract §6.2):

- `Control::deletion_plan` (preview: versions, files, bytes, assessments, latest-pointer
  moves; binding = the reports fence) and `Control::delete_versions` (one transaction:
  fence advanced first, rows removed, `report_version` and `report` tombstones with the
  job, latest pointer moved or cleared; already tombstoned versions skipped, an otherwise
  missing one refuses `conflict`).
- `Bundles::purge_deleted`: tombstoned bundle directories renamed out of the published
  tree (`.deleted-*`), synced, removed; leftovers of an interrupted removal removed.
  `Bundles::recover` runs it first, so recovery completes an interrupted deletion and
  never reattaches a deleted version.
- The lifecycle executor performs both steps; the revision scheduler skips reports with a
  `report` tombstone.

Tests:
- `tools::tests::report_artifacts_are_deleted_through_the_lifecycle_tools`: the grant
  gate refuses a non-administrator; an unknown version is `not_found`; the preview
  writes nothing and states files and the latest move; execution without proof is
  `forbidden`; with it a keyed retry joins the job; the bundle is removed, the other
  version stays and becomes latest, the deleted version is not retrievable; a stale
  preview after the fence advanced is refused.
- `tools::tests::an_interrupted_artifact_deletion_is_completed_by_recovery`: the control
  commit without file removal (a stopped process), then recovery: the bundle and a
  `.deleted-*` leftover are removed, nothing is reattached, the fence advanced, and a
  retry is a no-op success.
- `daily_store::tests::a_report_with_a_deleted_version_is_never_revised_again`.

Mutation checks (`m6r3/mutation-check.py`, log `m6r3/mutation-check.log`): fence not
advanced, files never removed, recovery not completing the deletion, latest pointer not
moved, no version tombstone, revisions ignoring the tombstone, a retry conflicting, and
binding the archive generation instead of the fence are all killed. Each half of the
last one alone survives only because the fixture's archive generation and fence coincide.

Limits: no real process kill between the control commit and the file removal (the
interrupted state is constructed directly); deletion is by version (deleting every
version of a report empties it but keeps its row and tombstone).

### M6 review 1 — 7.8/10, revise (preserved)

Source: `.local/dev/runtime-mcp/m6/review-1/result.md`. All 675 snapshot files verified
before and after.

The reviewer passed: HTML golden, determinism, escaping, CSP hashes, the Node filter
arithmetic, detail and byte thresholds, the existing DST and missed-tick tests, atomic
daily-slot association, and the schema 2→3 migration (`control_v2.sql` matches accepted
M5). Source inspection supports zero Reports calls for render, revision and retrieval;
per-request retrieval authorization; tenant-scoped lookup; assessment attribution and
hash binding; and enrichment quota reuse. The M5 follow-up tests are present.

Material findings:

1. Revision cutoffs admit later facts for existing events: `report.rs` filters
   `events.first_seen_ms` but reads current projections and unrestricted detector
   observations, so a +26 h fact changed the +4 h summary.
2. Already queued revisions can republish deleted report content: the revision executor
   checks the fence but not the report tombstone.
3. Spring-forward misclassifies a missed daily slot: catch-up is identified by 24 elapsed
   hours, so the missed March 8, 2026 slot (23 h old) becomes `daily`.
4. C6 recovery activates incompletely verified bundles: only `review.md` is verified
   before reattachment.
5. Publication completion events are not crash-safe: the event is inserted after the
   publication commit, recovery inserts none, and an identical-evidence retry returns
   before inserting it.

Minor findings: the page's JavaScript counters use ordinary objects (`__proto__` and
`constructor` application IDs break aggregation); the agent-facing summary counts capped
detail instead of the full cohort.

### M6 round 2 corrections (submitted for review 2)

The reviewer's six probes are kept as tests (`report::tests::review_later_facts_do_not_change_an_earlier_cutoff`,
`control::tests::review_c6_must_validate_all_bundle_files`,
`control::tests::review_c6_must_record_publication_completion`,
`control::tests::review_queued_publisher_must_obey_report_tombstone`,
`daily_store::tests::review_missed_spring_forward_day_is_catch_up`,
`report_html::tests::review_agent_summary_preserves_full_cohort_counts`, and the
collection test `review_queued_revision_cannot_recreate_deleted_content`).

1. **As-of reconstruction.** `repository::projection_as_of` computes an event's
   projection from the facts (and promoted receipt facts) first observed by a cutoff,
   read-only; `rebuild` writes the all-facts projection.
   - `report::late_as_of` finds the events first seen by the cutoff that have a fact
     first observed after it, and re-projects only those.
   - The summary SQL excludes them and merges their as-of projections (cells and
     detector cells, before the budget checks). Detail uses
     `repository::cached_event_as_of`, with facts by the cutoff only.
   - Coverage reads segments observed by the cutoff through the same connection
     (`inspect::coverage_groups_conn`), in the report's one read transaction.
   - Test: `an_earlier_cutoff_keeps_its_summary_and_detail_when_a_later_fact_arrives`.
2. **Tombstoned reports.** `record_version` refuses a report with a `report` tombstone
   inside its transaction; the executor also refuses at its start
   (`Control::report_deleted`).
3. **Calendar catch-up.** `daily_tick` classifies a missed slot as `catch_up` by its
   Chicago calendar date. Tests: the spring-forward probe, and
   `missed_fall_back_day_is_catch_up_and_today_is_daily`.
4. **Verified C6 reattachment.** `Bundles::verify_bundle` checks the required file set,
   each file's existence, regular type, size and SHA-256, no extra file, and the
   manifest's report, version and job identifiers, before `record_version`; otherwise
   the bundle is quarantined.
5. **Atomic completion events.** `record_version` inserts `report_published` in its
   transaction. `recover` runs `repair_publication_events` (idempotent), and an
   identical-evidence retry by the publishing job re-asserts the event. Test:
   `recovery_repairs_a_missing_publication_event`.

Minors:
- The page script's counters are null-prototype objects; test
  `the_filter_script_counts_prototype_named_applications` covers `__proto__`,
  `constructor` and `toString` under Node. The golden page changed only in the CSP hash
  and the two script lines.
- `reports::summary` takes the `ReportInput` and counts its full-cohort summary.

Mutation checks (`m6r4/mutation-check.py`, log `m6r4/mutation-check.log`): F1a–c (summary,
detail, whole read ignore late events), F2 (publication transaction ignores the
tombstone), F3 (24-hour catch-up), F4 (review.md-only verification), F5a/F5b (no in-transaction
event / no repair), m1 (ordinary-object counters) and m2 (capped-detail summary) are all
killed.

Limits:
- Late-event detection scans facts observed after the cutoff. It is cheap for the
  current version (no later facts), and proportional to later facts for an old
  revision; it is not re-qualified at a million scans.
- An event whose as-of time moves it out of the period is excluded by its as-of time.
- The executor-start tombstone refusal is defence in depth: the in-transaction check
  alone already keeps a queued revision from publishing, so no mutation isolates the
  executor refusal.
- C6 and event-repair tests build the crash state directly; no process kill was run.

### M6 review 2 — 8.5/10, revise (preserved)

Source: `.local/dev/runtime-mcp/m6/review-2/result.md`. All 675 snapshot files verified
before and after.

The reviewer confirmed the round-1 fixes:
- tombstoned reports are refused inside the publication transaction and by the executor;
- spring-forward and fall-back catch-up classify by Chicago date;
- every C6 artifact's size and hash are verified;
- completion events are atomic, and recovery repairs a missing one;
- both minors are fixed;
- ordinary later-fact reconstruction is correct.

It also confirmed artifact deletion, the control schema 2→3 migration, the
transaction-bound slot association, and no other M2–M5 regression.

Material findings:

1. Receipt reassociation can still change an earlier cutoff's summary: reconstruction
   uses the receipt's current `promoted_event_id`, so a competing event at +26 h that
   made the receipt ambiguous changed the +4 h summary.
2. Later facts can remove earlier detail through the size check: `repository.rs` counts
   all retained fact bytes before applying the cutoff, so 400 small later variants pushed
   an event over 256 KiB and dropped it from the +4 h reconstruction.
3. C6 recovery does not verify the manifest's job binding: any non-empty `job_id` is
   accepted, and the conditional event insertion then silently inserts no event.

Minor finding: recovered versions lose their agent-facing summary (initialised empty).
### M6 round 3 corrections (submitted for review 3)

1. **Receipt ownership as of the cutoff.** `repository::receipts_as_of` finds the
   receipts whose association could differ at the cutoff. Those are receipts with a
   receipt fact first observed after it, or whose scan has an event first seen, or given a
   fact, after it. For each, `receipt_owner_as_of` recomputes the event it was promoted
   to then, using `reconcile_scan`'s rule restricted to what was known by the cutoff: the
   receipt's own facts, the events first seen by then, and their facts' report IDs and
   scanner endpoints. Every event whose receipts then differ from now joins the late
   (rebuilt) set. The as-of projection (`projection_with`) and the detail
   (`cached_event_as_of`) read receipt facts through that association: unchanged receipts
   by their current owner, changed ones by their owner at the cutoff. Summary and detail
   therefore agree.
   - Tests: `review2_a_later_receipt_demotion_does_not_change_an_earlier_summary` (the
     reviewer's probe: a receipt promoted before +4 h, made ambiguous by an event at
     +26 h); `a_receipt_promoted_only_after_the_cutoff_does_not_change_it` (summary and
     detail).
2. **The size bound counts the facts the rebuild reads.** The 256 KiB bound in
   `cached_event_until` sums only the event facts first observed by the cutoff, plus the
   as-of receipts' facts first observed by it. Test:
   `review2_later_small_facts_do_not_remove_earlier_detail` (400 small later facts).
3. **The job binding is verified.**
   - **Recovery (C6).** Before reattaching a bundle, recovery requires
     `Control::report_job_binds`: the manifest's job exists, is this tenant's, and its
     canonical report parameters name the same report and kind. A mismatch is
     quarantined.
   - **Publication.** `record_version` refuses a version naming no job of the tenant,
     inside its transaction; the completion event is then inserted unconditionally.
   - Tests:
     - `review2_recovery_refuses_a_manifest_naming_an_unknown_job` (the reviewer's probe);
     - `recovery_refuses_a_manifest_naming_another_reports_job`;
     - `publication_refuses_a_version_without_a_job_of_this_tenant`.

     The test fixtures now publish through a real report job.

Minor: the full-cohort agent summary is staged in the bundle manifest (`x_summary`,
private, stripped from the public manifest) and restored on reattachment. Test:
`review2_recovery_restores_the_agent_summary`.

Mutation checks (`m6r5/mutation-check.py`, log `m6r5/mutation-check.log`; all 17 killed) re-run every
round-2 mutation (F1a–c, F2, F3, F4, F5a–b, m1, m2) with updated anchors, plus G1–G7
for this round:
- G1: no reassignment set;
- G2: as-of owner taken from the current association;
- G3: size bound over every byte;
- G4: recovery skips the binding;
- G5: binding ignores the report;
- G6: publication accepts a job of no tenant;
- G7: summary not restored.

Limits:
- **Cost.** The changed-receipt set and its recomputation scale with the receipts whose
  scan gained events or facts after the cutoff; this is not re-measured at 1M scans.
- **Approximation.** A receipt whose scan identity changes only through facts after the
  cutoff is recomputed from its earlier facts (correct); one that has no facts by the
  cutoff contributes nothing (correct). Report-ID and endpoint matching use only facts
  first observed by the cutoff, a conservative reading of `reconcile_scan`.

### M6 review 3 — 8.9/10, revise (preserved)

Source: `.local/dev/runtime-mcp/m6/review-3/result.md`. All 675 snapshot files verified
before and after.

All round-2 findings are corrected. The four retained probes passed independently, as did
wrong-report-job rejection and an additional eight-case receipt probe. Every available
round-1 probe still passes, and the reviewer confirmed scheduling, bundles, deletion,
assessments, zero Reports calls, the 2→3 migration and accepted M4/M5.

Material finding:

1. Manual report IDs conflate independent reports across recovery epochs:
   `reports_create` hashes tenant, caller and key but not the epoch, and report-row
   initialisation silently keeps an existing row. After an epoch change, a reused key for
   November 4 matched the November 3 report, and `enrichment_start` selected November 3.

Minor finding: the 8 KiB agent-summary ceiling counts characters, not bytes (12,237
bytes in 4,237 characters), in both generation and recovery.

### M6 round 4 corrections (submitted for review 4)

1. **Manual report IDs are scoped by the complete idempotency scope.** `reports_create`
   derives the report ID from a canonical JSON tuple of tenant, caller subject and client,
   recovery epoch and key. The same key text under a later epoch is a different report;
   a same-epoch replay is the same report and job; the old epoch's key is refused
   `control_restored`.
2. **No silent reuse of another report's row.** `Control::ensure_report` refuses
   `conflict` when an existing row under the ID describes a different report (tenant,
   kind, period, applications or title), instead of keeping it.
- **Minor: the summary ceiling is in bytes.** `reports::truncate_utf8` cuts the agent
  summary to `SUMMARY_MAX` (8 KiB) bytes at a character boundary, used by both generation
  and recovery.

Tests (`tools.rs`):

- `manual_report_ids_are_scoped_by_epoch`: same-epoch replay, old-epoch refusal, new-epoch
  separation, and conflicting row metadata refused.
- `the_summary_ceiling_counts_bytes_at_a_character_boundary`: 2- and 3-byte characters.

Mutation checks (each applied alone, all killed): E1 the epoch omitted from the report-ID
scope; E2 the row-metadata check disabled; E3 the ceiling counted in characters.

The epoch test also initialises both reports' rows and runs `enrichment_start` on the new
epoch's report: the job's period is November 4 (the review-3 probe selected November 3).

### M6 review 4 — 8.8/10, revise (preserved)

Source: `.local/dev/runtime-mcp/m6/review-4/result.md`. All 675 snapshot files verified
before and after.

The round-3 findings are fixed: the epoch-scope test passes in isolation, covering
replay, old-epoch refusal, distinct IDs, the enrichment period and conflicting-metadata
refusal, and multibyte summaries stay within 8,192 bytes in both generation and C6
recovery. Every earlier retained test passes.

Material finding (a round-4 regression):

1. The new metadata check rejects every scheduler-generated revision. Daily jobs carry a
   generated title, revision jobs carry `""` (`daily_store.rs`), so `ensure_report`
   reports a conflict and `Reporter::run` retries it every 60 s: scheduled revisions,
   including the final +24 h, never render. The existing revision test initialised the
   original with an empty title, which masked it.

No new minor findings.

### M6 round 5 corrections (submitted for review 5)

1. **Revisions revise the stored report.**
   - **Executor:** a revision always concerns an existing report, so the report executor
     uses that report's stored row and never re-initialises it from the revision job's own
     parameters. A revision whose report row is missing fails `not_found`. For other jobs,
     a row naming a different report is now a terminal `conflict`, not a 60 s retry.
   - **Scheduler:** `daily_store` revision jobs carry their original report's stored title
     and applications.
   - The round-4 collision guard for genuinely different reports is kept.

Tests:

- `collection::a_revision_with_other_metadata_revises_the_stored_report`: through the
  real report executor. An original published with title "test", then a revision job in
  the review-4 scheduler shape (empty title, no applications), succeeds against the stored
  report, which keeps its metadata. Reverting the executor fix makes it fail.
- `daily_store::revisions_coalesce_to_the_latest_due_opportunity`, made realistic: the
  original is initialised with its daily job's own generated title and applications, and
  each revision is asserted to carry them. Reverting the scheduler fix makes it fail.
- `collection::the_daily_schedule_publishes_a_daily_report` also queues and executes a
  scheduled revision when the published report is `daily`. That depends on the time of
  day, so the deterministic test above is the regression of record.

### M6 review 5 — 8.9/10, revise (preserved)

Source: `.local/dev/runtime-mcp/m6/review-5/result.md`. All 675 frozen files verified
before and after.

The round-4 scheduler blocker is fixed. The reviewer's scheduler-to-executor probe
covered a titled original, then +4 and delayed +24 revisions incorporating late evidence:
- counts went 1 → 2 → 3;
- the +25 h evidence was excluded;
- versions were distinct and predecessor-linked;
- the original bytes were unchanged;
- conflicting metadata was a terminal `conflict`.

Every earlier fix and accepted M4/M5 remain verified within the sandbox limits.

Material finding (pre-existing, newly identified):

1. Deduplication returns success without verifying the reused bundle: the
   unchanged-evidence path and the exhausted-revision-allowance path return `Succeeded`
   for the existing version without checking its files. With `report.html` removed, the
   unchanged-evidence +4 revision still succeeded. This violates the plan's "never return
   `succeeded` for an unverified or missing bundle".

Minor finding: a legacy queued revision (empty title) renders with the default title when
evidence changes, because rendering takes the title from the revision parameters instead
of the stored report.

### M6 round 6 corrections (submitted for review 6)

1. **A reused version is verified before success.** Both paths that return an existing
   version, unchanged evidence (deduplication) and an exhausted automatic-revision
   allowance, first run the full bundle verification used by C6 recovery:
   - the manifest lists every required file once;
   - each listed file is a regular file of the declared size and SHA-256;
   - there are no extra files.

   A missing or corrupted bundle fails `storage_unavailable` and the version is never
   reported as succeeded. The immutable version itself is left untouched.
- **Minor: rendering uses the stored report.** A revision renders and reads evidence
  with its stored report's title, period, applications and scheduled time; from the
  job it keeps only the cutoff and the final-revision flag.

Tests (`collection.rs`):

- `a_reused_version_is_verified_before_success`: an intact bundle succeeds with the
  existing version; a removed or corrupted `report.html` fails `storage_unavailable`; the
  exhausted-allowance path likewise refuses a missing file.
- `a_legacy_revision_with_changed_evidence_renders_the_stored_title`: a revision with an
  empty title and changed evidence renders a new version titled with the stored title.
- `scheduled_revisions_of_a_titled_daily_report_publish_through_the_executor`, the
  watchdog-requested scheduler-to-executor regression. A titled daily original, late
  evidence, then the scheduler's +4 revision carrying the title publishes a new immutable
  version, the +24 final revision runs, and the original's digest and manifest are
  unchanged. It fails on the review-4 code.

Mutation checks (each alone, all killed): V1 the deduplication reuse unverified; V2 the
allowance reuse unverified; V3 the title taken from the job parameters.

### M6 review 6 — 9.0/10, revise (preserved)

Source: `.local/dev/runtime-mcp/m6/review-6/result.md`. All 675 frozen files verified
before and after.

The round-6 corrections were verified:
- all 18 content-file reuse cases pass (intact, missing and same-size corrupted
  `report.html`, `evidence.json` and `review.md`, under deduplication and under allowance
  exhaustion);
- the changed-evidence legacy revision keeps the stored metadata;
- the actual scheduler/executor chain publishes the original, +4 and delayed +24
  versions (counts 1 → 2 → 3), excludes +25 evidence, keeps the original bytes, updates
  predecessor and latest links, and refuses conflicting metadata terminally.

Material finding:

1. Both reuse paths succeed with a missing or corrupted `manifest.json`: `verify_bundle`
   checks the three content files against the control-store manifest but only permits
   `manifest.json` without requiring, reading or validating it.

Minor finding: the retained scheduler regression does not keep the full final-cutoff
coverage (counts 1 → 2 → 3 with controlled observation times, +25 exclusion,
predecessor and latest assertions, original-byte comparison).

### M6 round 7 corrections (submitted for review 7)

1. **The on-disk manifest is verified before a reuse succeeds.** `verify_bundle` now
   checks `manifest.json` itself before it checks the content files:
   - it must be a regular file (not a link) of at most 1 MiB;
   - it must parse as JSON;
   - its public view must equal the control-store manifest;
   - its `x_evidence_digest` must equal the version's recorded digest.

   A missing, malformed or mismatched manifest fails `storage_unavailable` on both reuse
   paths. The immutable version is left untouched.
- **Minor: the stronger scheduler regression is retained.** A new hidden test helper,
  `airs_history_store::repository::seed_observed_event_for_tests`, records one synthetic
  event in its own completed sync run with a controlled observation time, following the
  crate's existing `*_for_tests` helpers.

  `scheduled_revisions_count_evidence_by_observation_time` drives the scheduler's own
  daily report for today's report-timezone period through the real executor. It seeds
  facts observed before the period end and at +2, +22 and +25 hours, then checks:
  - counts 1 → 2 → 3, with the +25 fact excluded from the final version;
  - `as_of` is the +24 cut-off;
  - each revision's predecessor, and the report's latest version;
  - the original's title on every version and in the revision parameters;
  - the original bundle's bytes, digest and manifest are unchanged.

  It replaces `scheduled_revisions_of_a_titled_daily_report_publish_through_the_executor`.
- **Test-order race removed.** The replaced test, and the round-4 scheduled-revision
  test, stepped the worker once and then read a specific job. A tick can queue several
  due jobs with one submission time. For example, the final tick also queues the next
  day's daily report. The worker orders such jobs by random ID, so roughly half of runs
  executed the other job first; this was seen as a failure in a scratch M7-stack run.
  Both tests now use `run_until_done`, which steps until the job under test leaves the
  queue. The new test asserts that the next-day report is queued. The product code is
  unchanged.

Tests (`collection.rs`): `a_reused_version_is_verified_before_success` covers seven
damages under both reuse paths (deduplication; exhausted allowance with late evidence):
- intact (succeeds with the existing version);
- `report.html` missing or corrupted;
- `manifest.json` missing, malformed, mismatched (`kind` changed), or with a changed
  `x_evidence_digest`.

Mutation checks (each alone, all killed):
- M1: the manifest check removed;
- M2: the digest comparison removed;
- M3: the public-manifest comparison removed;
- S1: the revision cut-off widened by two hours (the +25 fact counted);
- S2: the original's title not carried into revision parameters.

### M6 review 7 — 9.0/10, revise (preserved)

Source: `.local/dev/runtime-mcp/m6/review-7/result.md`. All 674 frozen files verified
before and after.

The round-7 corrections were verified:
- all 18 content-file cases pass;
- a missing, malformed or publicly mismatched manifest, and a changed evidence digest, are
  refused on both paths (additional same-size, missing-digest, symlink and oversized
  probes pass);
- the scheduler/executor chain gives counts 1 → 2 → 3, excludes +25 evidence, preserves
  the original bytes and title, and updates predecessor and latest (ten repeated runs);
- the test-order correction changes test execution only.

Material finding:

1. Both reuse paths accept corrupted recovery metadata. `public_manifest` also removes
   `x_fence` and `x_summary`, and neither was compared. A missing or changed `x_fence`
   or `x_summary` returned `Succeeded` under deduplication and under allowance
   exhaustion. In later C6 recovery, a fence-damaged bundle was quarantined, and a
   summary-damaged one was reattached with a wrong summary.

Minor findings: none.

### M6 round 8 corrections (submitted for review 8)

1. **The recovery fields are verified before a reuse succeeds.** `verify_bundle` takes
   the expected fence and requires:
   - `x_fence` to equal it;
   - `x_summary` to be a string equal to the stored version's summary. (The stored
     summary is the same bounded text the manifest records.)

   Both reuse paths go through `Reporter::reusable`. It reads the version's own
   publication fence from its `publications` row (new `Control::publication_fence`),
   then verifies against that fence. A missing publication row also refuses. Recovery
   passes the current fence, which it already requires the manifest to carry.

   A tenant fence advance after publication (prune or deletion elsewhere) therefore
   leaves an intact version reusable. A missing, malformed or changed recovery field
   fails `storage_unavailable`.

Tests (`collection.rs`, `a_reused_version_is_verified_before_success`): the damage
matrix adds five cases, under both reuse paths:
- `x_fence` missing or changed;
- `x_summary` missing or changed;
- the positive control: intact after an unrelated fence advance, which succeeds with
  the existing version.

Mutation checks (each alone, all killed):
- F1: the fence comparison removed;
- F2: the summary comparison removed;
- F3: reuse checked against the current fence instead of the publication fence (the
  positive control fails).

### M6 acceptance (2026-11-06T22:12Z)

The owner lowered the acceptance threshold to 9.0 (AGENTS.md). Reviews 6 and 7 each
scored M6 9.0, so M6 is accepted on review 7. Review 7's one material finding (unchecked
`x_fence`/`x_summary`) was corrected in round 8, which passed the required checks
(m6/run10: 4153/0/8, parity ran). Review 8 was stopped before it finished.

## M7 — Optional CLI client

Status: **ACCEPTED, 9.0/10 (review 11, 2026-11-07T03:46Z) under the owner-set 9.0 threshold.** Reviews 1–10 scored 7.0, 8.0, 8.5, 8.8, 8.9, 8.9, 8.8, 8.8, 8.9 and 8.9.

### M7 implementation (submitted for review 1)

- **M7.1, connections and sign-in.**
  - `airs-runtime-core::tenant::mcp` defines the strict `mcp` namespace, which
    `TenantConfig` parses, writes and validates.
  - `airs mcp connections set|list|remove` writes through the tenant transaction, with a
    `pre-mcp` rollback snapshot on the first write.
  - New production crate `crates/airs-mcp-client`, from the M0.6 spike, with no SQLite and
    no server dependency:
    - URL and origin trust, with a request-layer origin guard on every OAuth and discovery
      request;
    - the private session cache bound to resource, client and connection revision (0700
      and 0600, refusal of symlinks, hard links, oversize files and loose modes, atomic
      writes, a `flock` refresh guard);
    - PKCE `login`, noninteractive `access_token`, bounded typed `call`, and durable
      operation `receipts`.
  - `airs mcp login|logout|status`.
- **M7.2, the read and submit adapters.**
  - Reads: `runtime history status|coverage`, `runtime jobs list|get|cancel` and
    `runtime schedules list`.
  - Keyed: `runtime schedules set|disable` (epoch from `service_info`) and `runtime sync`,
    with a receipt before the send and `--wait`/`--no-wait`. An interruption prints the
    job ID and does not cancel.
  - Every service command requires `--mcp` (otherwise exit 2, naming the service).
- **M7.4, docs.**
  - DESIGN §12 rows D350–D354, the COMPATIBILITY §12 checklist, contract §7.5, DESIGN
    §22.8 and `docs-site/docs/commands/mcp.md`.
  - The regenerated command reference.
- **Graph gate.** `tests/no_local_history.rs` matches exact crate names: the CLI may name
  `airs-mcp-client` but never `airs-mcp`, `rusqlite` or `airs-history-store`.
  `cargo tree -p airs-cli -e normal` has no SQLite, history store or server.

Tests:

- `tests/mcp_client.rs` drives the real `airs` binary, through a fixture gateway and
  authorization server, to the real `airs-mcp` server:
  - `connections_are_strict_tenant_namespace_entries_with_a_rollback_snapshot`;
  - `login_then_history_jobs_sync_and_schedules_through_the_gateway`:
    - a noninteractive refusal with no sign-in started;
    - PKCE login and private session modes;
    - history, jobs and schedules calls;
    - sync with its receipt;
    - cancel;
    - logout, after which calls are refused;
  - `service_commands_need_an_explicit_connection_and_never_fall_back`.
- Unit tests:
  - `airs-mcp-client`: trust, cache privacy, binding, refresh lock, result bound,
    receipts;
  - `airs-runtime-core::tenant::mcp`: round trip and rules.

Deferred (contract §7.5 lists them):

- `runtime report --mcp`, and the `--from-evidence` × `--mcp` conflict;
- `runtime history query|reports|download|import|export|prune|compact|backup|recover|restore`;
- `runtime receipts upload` and `--record-history`;
- `airs mcp confirm` and admin connections;
- the bounded authentication-recovery replay of a submission;
- the frozen pre-M7 binary refusal test;
- packaged installs.

The parity tests need the pinned TypeScript reference (`.reference/`), which the isolated
copy lacks. They were not run here; the main tree's receipts must run them with
`AIRS_REQUIRE_PARITY=1`.


### M7 completion (submitted with review 1)

Every deferral of the slice is delivered except the live gateway proof of the
administrative step-up and the live/package proofs, which are M8 (contract §7.5 "Not
delivered"). DESIGN rows D355–D361.

- **M7.2 adapters.**
  - `runtime report --mcp`: `reports_create` under a receipted key, wait, then a
    hash-verified `--bundle-dir` or `--output-file`; the `--from-evidence`, `--review-file`
    and live-budget conflicts are usage errors before any tenant or network access.
  - `runtime history query|reports|download`.
  - `runtime history prune|compact|backup|recover|restore`: a privately recorded preview;
    `--execute` on an admin-role connection with a fresh, nonce-bound sign-in
    (`max_age=0`, `airs_proof_nonce`); `--execute --approval` on the previewing
    connection; `airs mcp confirm` (its own fresh, nonce-bound sign-in).
  - `runtime history export|import` (admin-role only): the whole flow in one command.
- **M7.3 retention and transfer.**
  - `runtime receipts upload --from-bulk-state` and `--record-history` on
    scan/bulk-scan/resume-poll: `client_asserted` receipts of the tenant's cloud ID,
    ≤200 records/256 KiB per batch under deterministic keys, a 2 s budget and a
    warning-only failure for `--record-history`, never a sign-in, never an AIRS call.
  - Chunked transfers verify every chunk's offset, length and SHA-256, the file SHA-256
    and the manifest length, retry a chunk at most three times from its verified offset,
    and publish without clobbering.
- **Contract §7.4 recovery.** One refresh of a refused session token per submission (at the
  epoch read or the send), replaying the original key.
- **Shared identity.** `airs-runtime-core::cloud` computes the archive's environment and
  cloud IDs; `airs-history-store::identity` now delegates to it (same values), so the CLI
  names the archive without the history store.
- **Grammar revisions** (contract §7.5): `--version-id` (the CLI's `--version` pre-scan),
  `confirm --digest/--proof-nonce`, `--approval`, `--no-wait`, `--open/--port` on admin
  commands, the report's existing destination flags, no controlling-terminal check (a pty
  is not a proof), `--record-history` needs `--scanner`.

Tests (`tests/mcp_client.rs`; the fixture backend now runs the service's worker with the
collection, report and lifecycle executors, `[admin] mode = "fresh_proof"`, a named
scanner with its receipt association, and Administrator grants for an admin and an agent
client; the fixture gateway signs `proof_nonce` and `auth_time` only for a fresh,
nonce-bound sign-in):

- `receipts_reports_and_downloads_through_the_gateway`: retention without a session warns
  and starts no sign-in; upload of a checkpoint (2 accepted) and its idempotent retry (the
  stored result); `bulk-scan`/`scan --record-history` retained with no warning; zero AIRS
  calls for uploads and for the report; the bundle's files match the manifest hashes;
  `download --file review.md` writes only that file into a 0700 directory and a second
  download into it is refused; `--record-history` without `--scanner`, `--from-evidence`
  and `--max-pages` with `--mcp` are usage errors before any sign-in.
- `admin_lifecycle_needs_a_fresh_nonce_bound_sign_in_and_keeps_no_session`: the admin
  connection refuses ordinary commands; a preview signs in without a proof; `--execute`
  signs in with exactly the preview's nonce and `max_age=0` and succeeds; a consumed or
  foreign preview cannot be executed; an agent's prune preview is approved by `mcp confirm`
  (nonce-bound) and executed with `--approval`; export (2 sign-ins) matches its job's
  SHA-256 and is 0600, a second export is refused before any sign-in; import of that
  export (3 sign-ins) succeeds; exactly five proof sign-ins; no admin session written.
- `a_rejected_token_is_refreshed_once_and_the_submission_replayed_with_its_key`: one
  refresh, one job, no token printed; with the refresh revoked too the submission fails
  without a sign-in and without a new receipt.
- `a_frozen_pre_m7_binary_refuses_the_mcp_table` (runs with `AIRS_PRE_M7_BIN`, built by
  `scripts/build-pre-m7-cli.sh` from the 8.0.0 release commit `f3e61ac`). Run in this copy
  against that binary (`airs --version` 8.0.0): it exits 1 with "Cannot read a valid tenant
  config" and leaves the tenant file unchanged. The default workspace run skips it.
- `packages_carry_the_client_in_airs_and_never_the_server`.
- Unit: `mcp_ops::tests` (batch bounds, deterministic keys, receipt normalization),
  `airs-mcp-client::previews::tests`.

Remaining limits:

- The deployed gateway does not sign the fresh proof (M0.6 P3); the admin flows are proven
  against the loopback fixture's model of it only.
- A download interrupted mid-command is rerun from the start (no cross-command resume).
- The packaged-install check is source and executable level; platform builds and
  installs are M8.4.

### M7 review 1 — 7.0/10, revise (preserved)

Source: `.local/dev/runtime-mcp/m7/review-1/result.md`. All 686 frozen files verified.

Material findings:
1. MCP tokens are not registered with the invocation's `SecretSet`, so an echoed token
   is not redacted, and refusals interpolate server-supplied text unfiltered.
2. Receipts are deleted before results are delivered: a stdout failure or a
   strict-partial exit loses the result after its receipt is gone.
3. Schedule mutations bypass durable receipts and the refresh-and-replay recovery.
4. Wait deadlines and cancellation do not cover the outstanding poll request.
5. Single-file report retrieval does not require a manifest entry, hash and length.
6. Import failures after staging (final preview, fresh sign-in) skip the upload abort.
7. Range validation can overflow (`--interval i64::MAX --unit days` exits 101), and a
   reversed time range reaches tenant loading.
8. An unrelated sign-in callback with a mismatched state aborts sign-in, where contract
   P5 requires ignoring it.

Minor findings: an exhausted report-list page budget is reported as "no such version";
the replay test does not cover lost responses after admission; D350–D361 lack `dNN_`
dual-CLI tests.

### M7 round 2 corrections (submitted for review 2)

Each finding has a discriminating regression (`tests/mcp_client.rs` unless noted).
Every mutation listed was checked alone and caught.

1. **MCP tokens are registered and every output is screened.** `run` (and
   `record_history`) binds the invocation's `SecretSet`; clones of a `SecretSet` share one
   set. Every MCP credential is registered where it is obtained: the cached or refreshed
   session token in `outcome`, the fresh administrative token in `auth_for`, and the
   refresh in `refresh`. Refusal reason and code, client error strings and server-supplied
   preview fields are screened. `emit` and the final failure (`with_secrets`) already
   consult the same set.
   - Test: `an_echoed_mcp_token_is_never_printed`. A hostile hop echoes the caller's
     bearer token in a refusal code and in a result; no issued token appears on stdout
     or stderr.
   - Mutation: registration removed (the token is printed).
2. **A receipt is removed only after its result reached stdout.**
   `Admitted::deliver(inv)` commits stdout, then removes the receipt. A closed pipe keeps
   the receipt and is success. A failed or interrupted write keeps it and is reported.
   Strict-partial and failed-job exits return after delivery, so the written result
   survives the failure exit.
   - Test: `a_receipt_outlives_an_undelivered_result_and_strict_exits_keep_the_result`.
   - Mutations: removal regardless of the write; removal without a commit; strict exit
     before delivery.
3. **Schedule changes use the keyed-submission path.** `schedules set` goes through
   `submit`, which gives it a durable receipt, one refresh and a replay with the original
   key, and delivery.
   - Test: `schedule_changes_keep_receipts_and_recover_authentication_once` (revoked
     token: one refresh, never a sign-in, one schedule; closed stdout keeps the receipt).
   - Mutation: the old direct call.
4. **The wait races the whole poll.** Authentication, connection and `jobs_get` race the
   absolute deadline and cancellation.
   - Test: `a_stalled_poll_cannot_outlast_the_wait_deadline` (a 60 s stall at the hop;
     `--timeout 2` ends near its deadline with the job ID, and the receipt is kept).
   - Mutation: plain await (killed by the harness at 30 s).
5. **Report retrieval requires a valid manifest entry.** `manifest_entry` is shared by
   single-file and bundle retrieval. It requires the entry, a 64-hex SHA-256 and a length
   within 64 MiB. The download is verified against both before publication.
   - Tests: `a_manifest_entry_needs_a_valid_hash_and_length` (unit) and
     `a_single_report_file_is_not_published_without_its_manifest_hash` (a hop strips the
     hash; nothing is published).
   - Mutation: the old optional-hash lookup.
6. **Import aborts the staged upload on every pre-execution failure.** This covers the
   final preview and the fresh execute sign-in. Execution is reconciled, not aborted.
   - Test: in `admin_lifecycle_needs_a_fresh_nonce_bound_sign_in_and_keeps_no_session`, a
     declined execute sign-in after staging gives exactly one `transfer_upload_abort`.
   - Mutation: no abort.
7. **Ranges are checked before tenant access.** `utc` returns the instant (it returned
   `0`). `--since` must precede `--until`. The interval uses `checked_mul`. The 90-day
   report bound also applies to explicit ranges.
   - Test: `invalid_report_and_history_ranges_are_usage_errors_before_tenant_access`
     (exit 2 with the specific message and no tenant: overflow, 91 days, reversed, a
     long explicit range, an empty history range).
   - Mutations: no ordering check; unchecked multiplication (panic, 101).
8. **A mismatched sign-in callback is ignored.** In `airs-mcp-client`, the state is
   validated inside the bounded callback loop. A wrong-state, stateless or wrong-state
   `error` callback gets a 400 and its code is never exchanged. The matching callback
   completes. Only wrong callbacks end at the existing timeout.
   - Unit tests: `an_unrelated_callback_is_ignored_and_the_matching_one_is_used`,
     `only_unrelated_callbacks_end_at_the_deadline`.
   - Mutation: take-the-first-callback (both fail).

Minor findings:
- An exhausted 200-page report search now reports an incomplete search, not "no such
  version".
- `a_lost_response_after_admission_replays_the_same_key`: the hop forwards the
  submission and loses its response. The hop records both submissions' idempotency keys
  and asserts they are equal; there is one job. (Mutation: replay with a new key. The job
  count alone could not catch it, because the service joins equivalent collections.)
- **Deferred:** the `dNN_` dual-CLI tests for D350–D361. The TypeScript reference has no
  MCP client, so these would assert the reference's refusal against the Rust behaviour.
  Not done in this round.

### M7 round 2 follow-up (after the review-2 freeze)

The disclosed deferral is closed. `tests/parity_mcp_client.rs` adds `d350_`–`d361_`
dual-CLI tests. Each runs one representative invocation and asserts:
- the Rust exit code and message;
- the TypeScript reference's unknown-command or unknown-option refusal;
- zero requests to the fixture service.

The 12 tests pass with `AIRS_REQUIRE_PARITY=1`. The `docs/COMPATIBILITY.md` §12 rows cite
them. This change is tests and documentation only; no product code changed. Review 2 scores
the frozen round-2 source, so it does not include these tests.

### M7 review 2 — 8.0/10, revise (preserved)

Source: `.local/dev/runtime-mcp/m7/review-2/result.md`. All 686 frozen files verified.

Round-1 corrections 2 (strict and closed pipe), 3, 4, 5, 7 and 8 were confirmed, along with
the pagination minor and the same-key replay check.

Material findings:
1. MCP-only invocations do not enable the final error filter (`lib.rs` applies it
   conditionally), and wait/interrupt diagnostics interpolate unscreened job IDs. A
   token echoed as a job ID reached stderr.
2. `--record-history` derives `scan_id` and `receipt_key` from the raw response scan ID
   without consulting the `SecretSet`. A credential with UUID syntax reaches
   `receipts_ingest`.
3. Import cleanup misses pre-send submission failures (the epoch read, the receipt
   limit).

Minor findings: the dual-CLI tests (closed by the follow-up above, after the freeze); a
failed export's result is discarded; `docs-site/.../mcp.md` says only `login` is
interactive, but administrative commands start a fresh sign-in.

### M7 round 3 corrections (submitted for review 3)

Each fix has a discriminating regression. Every mutation listed was checked alone and
caught.

1. **The final failure of every MCP invocation is filtered, and job IDs are validated.**
   - `bind_guard` turns on `guard_local_output`, so `lib.rs` filters the final failure
     even on an MCP-only tenant.
   - `valid_job_id` requires `j1_`, an 8-hex epoch and a lowercase UUID. `submit` checks
     the job ID before persisting it; an invalid ID fails, and the key-only receipt is
     kept. `wait` checks it before any diagnostic interpolates it.
   - Test: `a_token_echoed_as_a_job_id_is_never_stored_or_shown`. A hop prefixes the job
     ID with the caller's token: "invalid job ID", no token in stdout, stderr or any
     receipt file.
   - Mutation: no validation (the token is persisted).
2. **No outgoing receipt carries a known secret.** `retain`, the single path for
   `receipts upload` and `--record-history`, drops every record whose JSON carries a
   registered secret. The count is reported as `withheld` (in the upload result and in
   the `--record-history` warning).
   - Test: `record_history_never_sends_a_credential_returned_as_a_scan_id`. The scanner's
     UUID-shaped API key comes back as the scan ID: a warning, and the hop never sees
     the key.
   - Mutation: no filter.
3. **Import aborts the staged upload when execution was never sent.** `submit_tracked`
   reports whether a failure can have reached the service:
   - `Sent::No`: the epoch read or the local receipt failed;
   - `Sent::Refused`: the service created nothing;
   - `Sent::Uncertain`: a lost response or a failure after admission.

   Import aborts on `No` or `Refused` and leaves `Uncertain` to the receipt. `submit` is
   unchanged for every other caller.
   - Test: in `admin_lifecycle_needs_a_fresh_nonce_bound_sign_in_and_keeps_no_session`,
     the execute submission's epoch read fails after staging: exactly one abort.
   - Mutation: no abort.

Minor findings:
- The D350–D361 dual-CLI tests are included (see the round 2 follow-up).
- A failed export delivers its result before the failure exit, as the report path does.
- Docs: `docs-site/docs/commands/mcp.md`, the `mcp login` help text (and the regenerated
  reference) and DESIGN D351 distinguish ordinary session commands from administrative
  commands, which start their own fresh, nonce-bound sign-in.

### M7 review 3 — 8.5/10, revise (preserved, 2026-11-06T23:51Z)

Source: `.local/dev/runtime-mcp/m7/review-3/result.md`. The review-2 probes pass. The
pre-send import cleanup, the parity coverage and the failed-export delivery were
confirmed.

Material findings:
1. `scan` lowercases the returned scan ID before building its receipt, while the
   retention filter is case-sensitive. An uppercase credential reaches
   `receipts_ingest` in lowercase.
2. A token embedded as the UUID part of a syntactically valid job ID is persisted, and a
   later `mcp status` prints it.

Minor findings:
- No checked-in regression shows that the final diagnostic filter matters.
- No checked-in failed-export regression.
- No test of an authentication refusal at submission after a successful epoch read.
- A client diagnostic still says "no other command signs in".

### M7 round 4 corrections (submitted for review 4)

Every mutation listed was checked alone and caught.

1. **The outgoing filter ignores case.** The new `mc::carries_secret` checks exact and
   ASCII-case-folded forms. `retain` uses it, so a credential the CLI lowercased (scan
   IDs) is withheld.
   - Test: `record_history_never_sends_a_credential_returned_as_a_scan_id` now covers a
     lowercase and an uppercase UUID credential; the hop never sees either.
   - Mutation: the case-sensitive check.
2. **No job ID carrying a secret is persisted.** `submit` refuses a job ID that is
   malformed or carries a registered secret, so it never reaches a receipt.
   - Test: `a_token_inside_a_valid_job_id_is_never_persisted_for_a_later_status`. The
     token is the UUID part of a well-formed ID; neither that invocation nor a later
     `mcp status` prints it.
   - Mutation: syntax check only.

Minor findings:
- `a_timeout_diagnostic_never_prints_a_registered_token`: the service reports the token
  as the job state, and the MCP-only timeout failure is withheld whole. The job state is
  deliberately not screened in the diagnostic, so this test shows the final filter itself
  is what protects it. (Mutation: no `guard_local_output` in `bind_guard`.)
- A failed export, inside the admin test: the result reaches stdout, exit 1, its receipt
  is removed and no file is written. (Mutation: the old `inspect_err` path.)
- `a_submission_time_authentication_refusal_replays_the_same_key`: a 401 at the
  submission, after a good epoch read, gives one refresh and a replay with the
  original key. (Mutation: a new key.)
- The client's "authentication required" diagnostic now says no other command signs in
  only on an ordinary connection.

### M7 review 4 — 8.8/10, revise (preserved, 2026-11-07T00:18Z)

Source: `.local/dev/runtime-mcp/m7/review-4/result.md`. The round-4 corrections pass
independent offline probes.

Material finding:
1. Lifecycle preview records are persisted before their server-supplied fields are
   checked. A token in `expires_at` was not shown by the creating invocation, but a
   fresh `mcp confirm`, which knows nothing of that token, printed it.

Minor findings:
- The `airs-mcp-client` module documentation still says every command except `login`
  is noninteractive.
- The pre-M7 refusal test passes when its binary is missing, so an `ok` does not prove
  that it ran.

### M7 round 5 corrections (submitted for review 5)

1. **Preview records have their contract form and carry no secret.**
   `previews::well_formed` requires:
   - the format and a `p1_` preview ID;
   - an action of `[a-z_]` (at most 32);
   - a 64-hex parameter digest;
   - an optional 32-hex proof nonce;
   - an RFC 3339 UTC expiry.

   `preview` refuses to record anything that fails, or any field that carries a
   registered secret ("invalid preview; nothing was recorded"). `mcp confirm` and
   `--execute` refuse a stored record that fails the same form check before showing or
   using it. The form is the guard a later invocation has, since it does not know
   earlier tokens.
   - Test: `a_preview_carrying_a_token_is_never_recorded_or_shown_later`. A token in the
     returned expiry means nothing is recorded. A token planted in a record on disk is
     refused by a fresh `mcp confirm`, with no sign-in and nothing printed.
   - Mutations, each alone and caught: P1, no check before persisting; P2, no check on
     load (confirm prints the record and waits for a sign-in, and the harness kills it).

Minor findings:
- The client module documentation now qualifies the noninteractive rule to ordinary
  connections, and describes the admin sign-in.
- `AIRS_REQUIRE_PRE_M7=1` makes the pre-M7 refusal test fail when its binary is
  missing, and a run that executed it prints `PRE-M7 RAN` (`docs/TESTING.md`). The
  receipts set it whenever `AIRS_PRE_M7_BIN` is given. They record `pre-m7.txt`: the
  binary path, its `--version`, its SHA-256, and the `PRE-M7 RAN` line of a
  `--nocapture` run.

### M7 review 5 — 8.9/10, revise (preserved, 2026-11-07T00:52Z)

Source: `.local/dev/runtime-mcp/m7/review-5/result.md`. The expiry probe is fixed, and
the pre-M7 evidence gap is closed (the binary's hash and version were verified, and its
refusal was exercised independently).

Material finding:
1. Syntax validation cannot prove that an older record was screened. A UUID-shaped token
   fits inside a valid preview ID, and an alphabetic one fits the action grammar.
   Legacy records written before the persistence guard can still leak through a fresh
   confirmation. In addition, removing only the write-time secret check left the
   checked-in regression passing.

Minor finding: a `login` doc comment still says "the only interactive command".

### M7 round 6 corrections (submitted for review 6)

1. **Legacy preview records are never read.** The record format is now
   `airs-mcp-preview/2`. Only records whose every field was checked against the
   writing invocation's secrets carry it. `get`, `find` and `well_formed` accept only
   that format, so a version-1 record is invisible: `mcp confirm` asks for
   `--digest`/`--proof-nonce` without showing anything, and `--execute` finds no
   record. A preview is simply recreated.
   - Test: `a_preview_carrying_a_token_is_never_recorded_or_shown_later` now also
     covers:
     - a planted version-1 record whose preview ID carries the token as its UUID
       (exit 2, nothing shown, no sign-in);
     - a new preview whose returned ID carries the token in a valid shape (nothing
       recorded);
     - no token in any of the four invocations' output.
   - Mutations, each alone and caught: V1, accept any format (confirm shows the legacy
     record and waits for a sign-in); V2, no write-time secret check (the valid-shaped
     ID is recorded).

Minor finding: the `login` doc comment now says it is the only interactive command on an
ordinary connection.

### M7 review 6 — 8.9/10, revise (preserved, 2026-11-07T01:20Z)

Source: `.local/dev/runtime-mcp/m7/review-6/result.md`. The review-5 preview leak and
the comment are fixed, and both preview guards were shown to matter. There are no minor
findings.

Material finding:
1. Legacy operation receipts written before the job-ID guard are read without
   distinction. A later `mcp status` prints their `jobId`, which can carry a token in a
   valid shape.

### M7 round 7 corrections (submitted for review 7)

1. **A legacy receipt never shows its job ID, and stays unresolved.** Receipts are
   written as `airs-mcp-op/2`, after the job-ID secret check. `mcp status` shows:
   - a receipt's job ID only when it is version 2 (`Receipt::trusted_job_id`);
   - its key only in the CLI's own `cli-<32 hex>` form;
   - its operation only as a tool name;
   - and marks legacy receipts `"legacy": true`.

   A version-1 receipt stays listed as unresolved with its key and operation, so a
   possibly admitted submission is not silently dropped. Previews and receipts are the
   only stores of server-derived values a later invocation prints. Sessions are never
   printed, and downloads are verified files.
   - Test: `a_legacy_receipt_stays_unresolved_but_never_shows_its_job_id`. A planted
     version-1 receipt with the token as its job's UUID, beside a current `--no-wait`
     receipt: both stay unresolved, the legacy key is kept, its job ID is null, the
     current job ID is shown, and no token is printed.
   - Mutations, each alone and caught: R1, show the stored job ID (the token is
     printed); R2, write version 1 (the current job's ID is withheld).

### M7 review 7 — 8.8/10, revise (preserved, 2026-11-07T01:43Z)

Source: `.local/dev/runtime-mcp/m7/review-7/result.md`. The review-6 receipt
disclosure is fixed.

Material findings:
1. Verified downloads pass decoded bytes to the destination without secret screening. A
   hash-valid file containing a registered token was saved. This affects downloads,
   report files and bundles, and streamed exports.
2. `mcp status` turns receipt open or list errors into an empty array, and silently skips
   invalid receipts. A loosened directory (0770) or a malformed sibling receipt showed
   `"unresolved": []` with exit 0.

Minor finding: the contract still specifies `airs-mcp-op/1` writes; DESIGN D354 and its
mapping omit the round-7 behaviour.

### M7 round 8 corrections (submitted for review 8)

1. **No transferred byte carrying a secret is written.** `fetch`, the one path for
   artifact downloads, report files and bundles, and `export_read`, screens each chunk
   with the previous chunk's tail (`screened_window`). The tail is the longest
   registered secret form, minus one byte, so a secret split across chunks is found
   case-insensitively before the chunk reaches its sink. A carrying file is refused and
   nothing is published. Hash-bound bytes are never altered.
   - Tests: `a_transferred_file_carrying_a_token_is_never_published` (a report titled
     with the caller's access token: the download is refused and no file is written);
     unit `a_secret_split_across_chunks_is_found` (every split point; the tail stays
     bounded).
   - Mutation T1: no screen.
2. **`mcp status` never shows a partial inventory as complete.** `Receipts::inventory`
   returns the readable receipts and a count of unreadable ones. `status` fails, and
   removes nothing, when a receipt directory cannot be opened (for example, it is no
   longer private) or any receipt cannot be read. An absent directory is simply empty.
   - Test: `status_refuses_an_incomplete_receipt_inventory` (a malformed sibling, then a
     0770 directory: exit 1 each time, and the receipt is kept).
   - Mutations T2 (ignore unreadable receipts) and T3 (an open error read as empty).

Minor finding: RUNTIME_MCP_CONTRACT §7.4 documents `airs-mcp-op/2` receipts,
`airs-mcp-preview/2` previews, legacy handling, status failure and transfer screening.
DESIGN D354 and COMPATIBILITY D354/D356 map the new behaviour and regressions.

Each mutation was checked alone and caught.

### M7 review 8 — 8.8/10, revise (preserved, 2026-11-07T02:20Z)

Source: `.local/dev/runtime-mcp/m7/review-8/result.md`. The review-7 probes now
discriminate correctly: stable tokens, whole and split across chunks, are rejected on
every transfer path.

Material findings:
1. The overlap was taken once, before the transfer. A chunk request can register a
   longer token, which then escaped across a boundary. A bundle published an earlier
   file before a later file revealed the token.
2. `status` used `Path::exists()`, which is also false on permission errors. A
   `receipts` parent without search permission read as empty.

### M7 round 9 corrections (submitted for review 9)

1. **The screen follows the registry.** `fetch` takes the overlap again for every chunk
   and never below `MIN_OVERLAP` (16 KiB), so a token registered later, of up to that
   length, is still found across a boundary. Every in-memory file is screened whole
   (`clean`) against the final registry before publication. A bundle reads and verifies
   every file before publishing any. An export re-screens its temporary file, in
   bounded chunks, when the registry grew while it streamed (`rescan`; `Reservation::temp_path`
   is now public).
   - Test: unit `a_token_registered_mid_transfer_is_still_found` (a 2,005-byte token
     registered after the first 70 KB chunk, split across the boundary, is found; the
     whole-file screen sees it; a clean file passes).
   - Mutations: U1, no minimum overlap; U2, no whole-file screen.
2. **Only an explicit `NotFound` is absent.** `absent` uses `symlink_metadata` and its
   error kind. A permission or other error on the store or an ancestor is an incomplete
   inventory.
   - Test: `status_refuses_an_incomplete_receipt_inventory` adds a `receipts` parent
     without search permission (exit 1, receipt kept).
   - Mutation: U3, `exists()`.

Each mutation was checked alone and caught.

### M7 review 9 — 8.9/10, revise (preserved, 2026-11-07T02:44Z)

Source: `.local/dev/runtime-mcp/m7/review-9/result.md`. Both review-8 findings are
closed: 27 transfer probes passed, and the unsearchable ancestor is refused.

Material finding:
1. Receipt batches could exceed the service's request limit. A record larger than
   256 KiB was still inserted into a batch, and the size accounting excluded the MCP and
   JSON-RPC framing. A records array of 261,981 bytes made a 262,310-byte request, which
   was rejected with HTTP 413.

Minor finding: the publication checks lacked adapter-level regressions. Disabling all
three final checks still passed the committed unit tests.

### M7 round 10 corrections (submitted for review 10)

1. **Batches fit the complete request.**
   - `batch_plan` sizes every candidate batch as the complete serialized `tools/call`
     request (JSON-RPC wrapper, call envelope with tenant, key and epoch, and the
     parameters, `request_bytes`), plus `REQUEST_SLACK` (2 KiB) for transport
     additions, against the 256 KiB limit.
   - A record too large to be sent even alone is never sent. It is counted as
     `oversized`, in the `receipts upload` result and in the `--record-history`
     warning, and the remaining records still go.
   - Keys stay deterministic. Retention stays warning-only, and no scan is resubmitted.
   - `retain` reads the epoch first, so the request size is exact.
   - Test: `every_batch_request_fits_the_limit_and_oversized_records_are_counted`. A
     records array just under 256 KiB, with fewer than 200 records, is split so that
     every complete request fits. One record over the limit, between two small ones, is
     counted and the other two are sent.
   - Mutations: B1, array-only sizing; B2, no oversized skip.
2. **Minor: adapter-level publication regressions.** The bundle, single-file and export
   publication steps are now the functions `publish_bundle`, `publish_single` and
   `finish_export`.
   - Test: `publication_refuses_a_token_registered_after_the_bytes_were_read`. A token
     registered after the bytes were read makes each step refuse and write nothing (no
     bundle directory, no file, no linked export). Clean controls publish.
   - Mutations P1 (no bundle screen), P2 (no single-file screen) and P3 (no export
     rescan) each fail it.
   - The tests that bind the process-wide registry are serialized by a lock.

Each mutation was checked alone and caught.

### M7 review 10 — 8.9/10, revise (preserved, 2026-11-07T03:20Z)

Source: `.local/dev/runtime-mcp/m7/review-10/result.md`. Both review-9 findings are
closed: the boundary batch becomes two HTTP-200 requests through the pinned transport,
and every mutation fails its regression.

Material finding:
1. Round 10's planner re-serialized every candidate batch, which is quadratic, and
   `retain` plans synchronously, so the Tokio timeout cannot interrupt it. A 10,000-record
   input took 3.58 s to plan; the round-9 batcher took 31 ms.

Minor finding: DESIGN D359/D360 and COMPATIBILITY do not describe the round-10
behaviour or map its regressions.

### M7 round 11 corrections (submitted for review 11)

1. **Planning is linear.** Each record is serialized once. A batch's request size is the
   fixed framing, measured once with a key of the final length, plus its records and
   separators. This is exact, because the parameters are embedded as JSON. Keys are
   computed only when a batch is final. Batch contents are unchanged: the round-10 size
   and oversized regressions pass unchanged.
   - Test: `planning_the_largest_input_fits_the_retention_budget`. 10,000 near-limit
     records plan in under eight times the cost of serializing each once, plus 100 ms,
     measured on the same machine and build so the bound holds under load. Every
     planned request fits. A debug build takes about 0.8 s.
   - Mutation L1, re-serializing the batch per record: 22.6 s against a 261 ms unit, so
     the test fails.
2. **Minor: specifications.** DESIGN D359 states complete-request sizing, the
   `oversized` and `withheld` counts and linear planning. D360 states the warning for
   any record not retained. COMPATIBILITY D356, D359 and D360 map the round-8 to
   round-11 regressions.

### M7 review 11 — 9.0/10, accept (2026-11-07T03:46Z)

Source: `.local/dev/runtime-mcp/m7/review-11/result.md`. All 686 frozen files verified
before and after. No material or minor findings are outstanding.

Verified:
- **Retention budget.** The 10,000-record probe plans in 31 ms. With a permanently
  pending send, retention returns in 2.024 s with one warning. The round-10 planner
  reproduces the failure, and the committed regression fails on it.
- **Round-9/10 batching boundaries.** Through the pinned rmcp transport they hold: a
  259,460-byte and a 3,180-byte request (both HTTP 200, all 184 records); the oversized
  record is counted and its neighbours are sent.
- **Specifications.** The D359/D360 text and the mappings are present.
- **Source review.** Explicit selection, no fallback, noninteractive ordinary
  authentication, fresh nonce-bound admin proof, one same-key recovery, verified
  no-overwrite transfers, and retention without scan resubmission. The CLI graph
  excludes SQLite, history-store and server. The server subtree matches accepted M6.

Stated limits: 21 integration tests and two callback tests were blocked by the review
sandbox's socket restrictions; the supplied exact-source receipts (4,207 passed, 0
failed, 8 ignored, strict parity, `PRE-M7 RAN`) cover them. HTTP 200 establishes
transport admission, not semantic ingestion. Packaged platform installs and the live
gateway proof are M8.

## M8 — Deployment, scale and release qualification

Status: ****source side ACCEPTED 9.0/10 (review 8, 2026-11-07T16:03Z)** under the 9.0 gate; overall M8 still pending the owner's live evidence (reviews 1–7 scored 6.5, 6.5, 7.5, 8.0, 8.5, 8.7 and 8.8)** (reviews 1–5 scored 6.5, 6.5, 7.5, 8.0 and 8.5);
integrated on accepted M7 (2026-11-07); awaiting user-run integration evidence (not
accepted). The plan's exit gate needs an
independent source and receipts review at or above the owner-set 9.0 threshold **plus**
actual integration evidence from the owner's live steps (`scripts/m8/user-checklist.sh`).
Nothing here is published, applied or deployed by an agent.

### M8 implementation

- **M8.1 artifacts and checks.**
  - `deploy/airs-mcp/` holds JSON (valid YAML) manifests: namespace, Certificate, PVC,
    Deployment, Service, NetworkPolicies, a configuration template, a kustomization and
    a Dockerfile.
  - The test `deploy_manifests.rs` checks the manifests statically:
    - singleton `Recreate`, non-root, read-only root filesystem, drop ALL, no `fsGroup`;
    - a CHOWN-only, non-recursive init container with a pinned digest;
    - secrets projected read-only at 0400 and never in the environment;
    - a 100 Gi `ReadWriteOncePod` PVC on the encrypted class;
    - `https` is the only Service port; ingress 8443 from the gateway only and 8081
      from `monitoring` only; no private-range egress;
    - the configuration template passes `config::parse`, and every listener and file it
      names matches a container port or mount (each credential file is projected);
    - the kustomization lists every manifest and refuses the placeholder image digest.
  - `/metrics` on the private health listener (`metrics.rs`, `run::health_router_with`)
    serves fixed Prometheus series: up, ready, maintenance, `control_ok` (a bounded 2 s
    control read), the ten job states, data-volume size and free bytes, and archive and
    control bytes. It has no tenant, path or credential data.
    - Unit tests: fixed series and labels, an unanswered control store, bounded size sums.
    - Loopback test `the_private_health_listener_serves_bounded_metrics`: real service
      storage, no tenant name or path, and maintenance turning `ready` off.
  - **Backup-policy checks (§9.4), user-run.**
    - `scripts/m8/check-r2-delete-rights.sh` is an offline validator of an owner-made,
      sanitized `INVENTORY.json` (credential names, kinds, permissions and scopes; never
      values). It makes no network call and reads no token; unknown entries block, and a
      modify permission whose scope covers the backup bucket is a violation. (As first
      implemented it inspected pasted tokens through two read-only API reads; source
      review 1 found that boundary wrong, and round 2 replaced it.)
    - The nas1 retention record is checklist step 4.
    - A repo test asserts that the R2 script stays read-only (no write methods, object
      operations or cluster commands) and that every M8 script parses.
- **M8.2 local qualification.**
  - `qualify_restore_and_contention_at_scale` (opt-in) seeds the qualified dataset and
    runs four 30-day report readers against a live writer, each within the report read
    budget. It then takes a `VACUUM INTO` backup copy, restores it into a fresh root,
    and checks `integrity_check = ok` and an exact event count. It records peak RSS and
    I/O from `/proc/self`.
  - Results are in the next section.
  - The node-loss test (dedicated disposable node), production-storage restore,
    low-space recovery on the actual storage class and PVC re-sizing from measured growth
    are user-run.
- **M8.3.** `docs/RUNTIME_MCP_DEPLOYMENT_RUNBOOK.md` gives eleven steps plus rollback.
  `scripts/m8/user-checklist.sh` records each step's result into a 0600 evidence file and
  refuses text that looks like a credential.
- **M8.4.**
  - `docs/RUNTIME_MCP_RELEASE.md` covers the service release. The CLI keeps
    `RELEASING.md`.
  - `scripts/m8/build-hashes.sh` builds `airs` and `airs-mcp` locally for the host and
    installed Linux targets. It writes `SOURCE-SHA256SUMS`, `BIN-SHA256SUMS` and
    `BUILD.txt`, and never signs, tags, pushes or uploads.
  - macOS signing and the container image are owner steps.

### M8.2 local qualification results (1,000,000 scans, release build)

Recorded in `docs/qualification/m8.2-restore-contention-1m.json`. The test directory was
tmpfs, so these qualify the software, not disk I/O. The storage-I/O counters are near zero
there by construction, and real-disk I/O on the encrypted Longhorn class is a user-run
measurement.

| Measure | Result |
|---|---|
| Seed 1,000,000 scans | 105.8 s |
| 50,000 more scans written while four readers each run three 30-day report inputs | 18.6 s; slowest read 5.34 s (report read budget 60 s); no read or write failed |
| `VACUUM INTO` backup copy | 2.59 s; 1.68 GB (live archive 1.92 GB) |
| Restore into a fresh root + `integrity_check` + exact count (1,050,000 events) | 14.3 s; `ok` |
| Peak RSS of the whole run | 215 MiB |

The archive is larger than B0's 756 MB per 1M scans because these synthetic scans carry
application names, sessions and detector hints for the report path. §9.5's 100 Gi
initial PVC still covers several million scans plus the 2× maintenance reserve; M8
re-derives the size from measured production growth (user-run).

### M8 user-run steps that remain (deployment is not accepted without them)

1. Fill the configuration placeholders (runbook step 1).
2. Build, push and pin the image (step 2).
3. Confirm the encrypted storage class, secrets and pull secret (step 3).
4. Record nas1 retention (step 4).
5. Prove no agent-accessible R2 delete right (step 5).
6. Deploy and check probes and metrics (step 6).
7. Gateway sign-in and service proof (step 7).
8. Claude Code proof (step 8).
9. Backup marker absence (step 9).
10. Node-loss test on a dedicated disposable node (step 10).
11. Restore drill (step 11).

### M8 source-side review 1 — 6.5/10, revise (preserved, 2026-11-07T09:35Z)

Source: `.local/dev/runtime-mcp/m8/review-1/result.md`. All 704 frozen files verified.
The missing live evidence did not reduce the score: the findings are defects in the
artifacts and procedures meant to produce that evidence.

Material findings:
1. Projected secrets are root-owned 0400 while the service runs as UID 65532 without
   `fsGroup`, so TLS and AIRS credentials cannot be read.
2. `deploy_manifests.rs` misses security regressions (public metrics or TLS ingress,
   unrestricted egress, a privileged container, a wrong certificate hostname all pass)
   and rejects a valid image pin.
3. `check-r2-delete-rights.sh` calls Cloudflare with bearer values in `curl` arguments,
   reads credentials despite claiming not to, and can succeed on incomplete inspection.
4. `user-checklist.sh` accepts `password=…` and `Authorization: Bearer …` and rejects
   SHA-256 digests; it has no executable tests, and status answers are unrestricted.
5. The backup marker procedure can certify encryption falsely (`receipt_key` is
   stored hashed), and omits the coordinated control/archive/artifact backup set of §9.4.
6. The restore drill starts the service before offline maintenance (which refuses while
   it runs), and does not handle the restored control epoch, quarantine, reconciliation
   or activation.
7. The live-proof recipe omits revision 7's dedicated-gateway-client checks. The
   node-loss test pins its workload to the lost node without a defined transition and
   fencing evidence. The low-space and capacity steps are missing.
8. `build-hashes.sh` exits 0 when every build fails.

Minor findings: metrics filesystem sampling is unbounded in time; the binary-hash
comparison lacks reproducible build conditions; a qualification comment overstates its
backup coverage.

### M8 round 2 corrections (submitted for source-side review 2)

1. **Secrets are delivered owner-only to UID 65532, with no `fsGroup`.**
   - An init container, `secrets`, runs as root with CHOWN only and is not privileged.
   - It alone mounts the raw Secret volumes (`tls-source`, `airs-secrets-source`, 0400).
   - It copies them into a memory-backed `emptyDir` as 0400 files in 0500 directories
     owned by 65532.
   - The service mounts only that volume, read-only, at the unchanged paths.
   - Rotation is a `rollout restart` with `Recreate` (deploy/airs-mcp/README.md).
   - Test: `the_secret_copy_yields_owner_only_files_readable_by_their_owner` runs the
     real init script.
2. **Manifest validation is discriminating and accepts a real pin.**
   - Every rule is a check over a directory, comparing complete effective
     NetworkPolicy rules: exactly four policies, a full default deny, TLS only from
     the gateway pods, health only from `monitoring`, three named egress rules,
     private ranges excepted, no IPv6 catch-all.
   - Also checked: hardening, the secret delivery, config/port/Service
     correspondence, and certificate `dnsNames` = `allowed_hosts`.
   - `every_security_regression_is_refused` applies 20 mutations to temporary copies,
     including the reviewer's five.
   - `validate_deployable` requires and accepts `sha256:` plus 64 lowercase hex.
     `AIRS_MCP_REQUIRE_DEPLOYABLE=1` validates the owner's filled set.
3. **The R2 helper is offline.** `check-r2-delete-rights.sh` validates an owner-made,
   sanitized `INVENTORY.json`. It makes no network call and reads no credential or
   environment, and every string goes through the secret check. Exit codes: 0 clean,
   2 invalid or secret-looking, 3 incomplete or uninspected, 4 violation.
4. **Checklist input is field-validated.**
   - Shared `lib-secrets.sh` detection covers PEM, JWT, `Authorization:`/Bearer/Basic,
     key=value secrets and AWS key IDs. It exempts digests and UUIDs, and long
     alphanumeric runs are refused.
   - Field kinds are text, hash and status (`pass`/`fail`/`skip` with a reason), with a
     `--check KIND` mode for tests.
   - Refused input is never echoed or recorded.
   - Test: `m8_scripts.rs`.
5. **The backup marker proves encryption, and the coordinated set exists.**
   - The marker is a report title, stored as written (`reports.title`), with a
     positive control.
   - A missing marker counts only with positive encryption evidence, a wrong-key
     refusal and a correct-key restore.
   - New `airs-mcp control-backup-set` writes the §9.4 set while stopped, under the
     owner lock: `VACUUM INTO` control and every archive, the artifact list, and
     `manifest.json` (`airs-mcp-backup/1`, with a documented `rows_digest`), all 0700/0600,
     no-clobber and fsynced. `control-backup-verify` re-hashes and recomputes it.
   - Its relation to the daily Longhorn backup and its retention is documented
     (runbook §9a/§9b).
6. **The restore drill is executable and handles control state.**
   - It runs in an isolated namespace with default-deny egress and placeholder secrets.
   - Order: Deployment at 0, restore, `control-backup` of the restored store, move it
     aside, `control-restore` (pending epoch), `control-activate-epoch`. Reconciliation
     is deliberately withheld, so quarantine prevents AIRS sends.
   - Then a controlled startup and marker verification. The production prerequisites
     are stated (runbook §11).
7. **The live proof states every assertion.**
   - §7a: dedicated confidential gateway client, sole consent, exclusive credential
     use, no other integration targeting the backend, with refusal conditions.
   - §10: node loss across two disposable nodes, with labels, taints and cordons, no
     force-detach while ownership is uncertain, power-off fencing evidence, RWOP
     attachment states and the new owner UUID.
   - §12: low-space behaviour and capacity qualification.
   - The disk reserve is implemented (§6/§9.5) by the new `space.rs`.
     Publication-class jobs pause below reserve + 512 MiB, and collection and
     enrichment below the reserve (`Outcome::LowSpace` → `blocked_policy`, requeued
     only when space allows). Receipts and upload staging return
     `storage_unavailable`. New metrics: `airs_mcp_storage_reserve_bytes` and
     `airs_mcp_storage_low_space{level}`.
   - Test: `the_disk_reserve_pauses_publication_then_collection_and_resumes`.
8. **`build-hashes.sh` fails on failure.** A build failure or a missing binary exits 1
   and writes no `BIN-SHA256SUMS`. A missing required target is recorded and exits 3.
   The full matrix is always recorded, ending `release-qualification: blocked
   (pending: …)` until complete.

Minor findings:
- Metrics filesystem sampling uses a per-store cache refreshed on the blocking pool,
  within 500 ms, plus availability and age gauges.
- `RUNTIME_MCP_RELEASE.md` records image-binary provenance (base image by digest,
  toolchain, target, sha256) instead of a host-versus-image comparison.
- The `report_tests.rs` comment now says "archive-only".

New owner helper (owner request): `scripts/m8/derive-config.sh` walks every
`airs-mcp.toml` value. For each it gives a read-only lookup and a suggestion, checks
CoreDNS, pihole and Cloudflare without changing them, writes
`.local/airs-mcp/airs-mcp.local.toml` (0600, gitignored), and refuses secrets.
Checklist step 12 is added.

Also, from the watchdog's round-2 check:
- **An unmeasurable volume fails closed.** When `statvfs` fails, publication and
  collection both pause until a measurement succeeds; lifecycle operations and reads
  still run. The reserve regression adds that stage (both stay paused, and a new
  collection pauses), and restoring the earlier fail-open behaviour fails it.
- **Binding documentation.** DESIGN §22.8 has an M8 entry: artifacts, the backup-set
  commands, the reserve and its fail-closed rule, metrics and the owner tooling. The
  CLI is unchanged, so there is no §12 row. COMPATIBILITY §12a maps each M8
  behaviour to its tests.

Each new guard was mutation-checked alone and caught: 20 manifest mutations, M3a–M8b,
D1–D2, R1–R4, and fail-closed.

**CI blockers fixed with round 2 (both pre-existing, outside M8 code):**
- *`signal_reads` deadline tests.* Wall-clock bounds of 2–5 s on whole CLI runs failed on
  the contended CI runner (PR #9 `linux`). Each bound is now 15 s, and each test still
  fails without its deadline: the fallback test uses `--timeout 30`, so its fallback
  must fire at 3 s.
- *The tenant-store locking flake* (`tenant_write locking::*`, documented since M4). A
  reader's `symlink_metadata`/`open`/`fstat` checks saw a different inode when a
  concurrent writer renamed a new file over the path, and refused the file as unsafe.
  `open_checked` now repeats the checks on a replaced file, up to 16 times, while a
  wrong type, owner or mode is still refused at once (DESIGN §3.8). Stress test: 40 runs
  of the locking group with the retry gave 0 failures; without it, 2/40.

### M8 source-side review 2 — 6.5/10, revise (preserved, 2026-11-07T11:08Z)

Source: `.local/dev/runtime-mcp/m8/review-2/result.md`. All 711 frozen files verified.
The review-1 fixes are confirmed. The store retry is bounded, and the §3.8 type, owner,
mode and symlink refusals are unchanged. The `signal_reads` bounds still discriminate.

Material findings:
1. The secret init container chowns each 0500 directory before its children, so root
   with only CAP_CHOWN loses search permission. The test substitutes the current UID.
2. Manifest validation does not prohibit extra egress on the ingress policies.
   `egress: [{}]` on the gateway policy was printed `DEPLOYABLE`.
3. `derive-config.sh` invokes service CLIs (az, kubectl, cf, airs) and can create a
   public DNS record; the helpers must stay offline.
4. `check-r2-delete-rights.sh` treats unknown permissions and scopes as clean (an unknown
   permission, and `s3:DeleteObject` on `arn:aws:s3:::…/*`, both exited 0). The runbook
   still describes the obsolete token-prompt workflow.
5. The coordinated backup suppresses archive discovery and stat errors, and skips
   malformed artifact manifests. It records the control schema as 0, not 3. Verify
   accepts a wrong epoch, a fabricated schema, and an invalid artifact list.
6. Backup hashing reads whole databases into memory (beyond the 2 GiB pod limit for
   multi-GiB archives).
7. The reserve fails open: every `lifecycle_execute` (including allocating
   export/import) is admitted; a reservation-query failure reads as zero; directory
   sampling suppresses errors.
8. Metrics start a fresh, blocking reserve measurement on every scrape (stalled probes
   accumulate), and request handlers await `space_bounded` without a deadline.
9. Proof steps are not executable as supplied: the checklist names the coordinated
   set before `control-restore`, which needs a control-only backup; the grants omit the
   report and job tools the proofs need; the low-space step reuses a quarantined drill
   with no authenticated route.
10. The daily backup procedure substitutes running-volume snapshots; contract §9.4
    requires the daily off-volume backup after the application set.

Minor findings: invalid derived TOML exits 0; `space.rs`'s module doc still says an
unmeasurable volume pauses nothing.

### M8 round 3 corrections (submitted for source-side review 3)

1. **The secret hand-off is completable with CHOWN only.** The init script chowns the
   files, then chmods each directory to 0500, then chowns the directories last, so root
   never needs a directory after giving it away.
   - A static order check refuses any later command on a child.
   - `the_secret_copy_never_needs_a_directory_after_giving_it_away` runs the real script
     with each hand-off modelled as a loss of access. The committed order passes and the
     review-2 order fails.
   - The README states what the step-6 live proof must still show.
2. **Every policy's spec is exactly one direction.** Each policy is its selector,
   `policyTypes` and one rule set (`["Ingress"]` or `["Egress"]`).
   - The mutation suite (now 32 cases) adds egress on the gateway policy (the
     reviewer's) and on the health policy, and ingress on the egress policy.
   - A fully pinned copy of every mutation is asserted not deployable.
3. **The derive helper is offline by default.**
   - By default it reads only local files and prints each lookup for the owner, and
     pasted results are screened.
   - `--run-lookups` runs read-only commands through an allowlist, refusing mutating
     verbs.
   - Public DNS creation is removed; the `cf` command is printed instead.
   - Values are serialized safely, and the output is published only after a TOML
     round-trip and structural check (when the local `python3` has `tomllib` or `tomli`)
     and `airs-mcp check-config`. Without a Python TOML parser the service parser is
     required rather than optional. Invalid output exits 1 and leaves no file.
   - Tests use service stubs that fail if invoked.
4. **The R2 checker has a strict schema.** It defines exact keys, six kinds, a closed
   Cloudflare permission list, the `s3:` action grammar and a scope grammar.
   - Unknown kinds, keys, permissions, scopes or sources exit 3.
   - A modify permission on a scope covering the bucket exits 4, including ARN prefixes.
   - The reviewer's two probes are tests. The runbook §5 now describes the offline
     inventory workflow.
5. **The backup set fails closed and records real metadata.**
   - Archive discovery errors are propagated, and malformed artifact manifests fail
     the set.
   - The schema is the control store's `meta.schema` (3) and each archive's
     `user_version`.
   - Verify checks the epoch against the control copy, each schema, the SQLite
     version, `created_at`, the artifact-list shape and duplicate store names.
6. **Hashing is streamed** through a 1 MiB buffer in creation and verification
   (`hash_reader`/`hash_file`). `rows_digest` reads through a cursor. RELEASE §1 adds a
   coordinated-backup qualification under the 2 GiB memory limit.
7. **The reserve has no fail-open path.**
   - `lifecycle_execute` is classified by action: `prune`, `compact` and
     `artifact_delete` are exempt; `export`, `import_finalize`, `recover` and unknown
     actions are publication-class. The worker reads the action from the job's
     parameters.
   - A failed reservation query, or a directory or entry error, gives `Unknown`
     (`store_bytes_checked`).
8. **One shared, bounded reserve sampler.** `Storage::space_sampled` keeps one cached
   measurement (2 s), runs at most one blocking measurement at a time, and waits at most
   500 ms; otherwise the answer is `Unknown`, so allocations fail closed. Both
   `space_bounded` (request handlers) and the metrics scrape use it.
   - Test: 4 sampler calls and 3 scrapes share one stalled probe, and each returns
     within the bound.
9. **The proof steps are executable.**
   - The checklist and runbook §11 use the control-only `control-backup` for
     `control-restore`, and the coordinated set for content.
   - The `airs-cli` grant covers the proof tools: `history_query`,
     `jobs_list/get/cancel`, `reports_create/list/get`, `artifacts_read/download` and
     `schedules_list`. Every granted tool must exist in the catalogue.
   - §12 is a separate qualification in its own `airs-mcp-qual` namespace, with an
     authenticated route and deliberate reconciliation, and no AIRS egress.
10. **The daily backup follows §9.4.**
    - `deploy/airs-mcp/backup-cronjob.json`: a 03:00 America/Chicago orchestrator
      (scale to 0, the set Job, scale back to 1 on any exit) with minimal RBAC, a
      suspended hardened set-Job template, and egress only to the API servers.
    - The set step runs `control-backup-set --keep 3`. The new set is verified, then
      only complete sets beyond the newest 3 and older interrupted runs are removed,
      never anything newer and never off-volume copies (`backup_set::prune`).
    - The verify step runs `control-backup-verify --latest-in`, because the distroless
      image has no shell to pass the set ID (`backup_set::latest_complete`).
    - Runbook §9a sets the schedule, a success gate (a day counts only if its set
      verified and the Longhorn backup ran after it), failure handling and retention.
    - Runbook step 2 pins the `kubectl` image as well.

Minor findings: derive output is validated before publication (above); `space.rs`
documents the fail-closed behaviour and its exemptions.

Each mutation was checked alone and caught:
- fork A: G1–G6, and 12 new manifest mutations;
- fork B: B1 and B3–B8;
- fork D: M1–M5 and M7–M10;
- retention: P1 (sets newer than the verified one pruned) and P2 (keep off by one).

### M8 source-side review 3 — 7.5/10, revise (preserved, 2026-11-07T11:52Z)

Source: `.local/dev/runtime-mcp/m8/review-3/result.md`. All 712 frozen files verified.
Most review-2 defects are fixed.

Material findings:
1. `derive-config.sh --run-lookups` still runs service commands. The offline
   requirement applies to the helper itself, not only its default.
2. `control-backup-verify` checks the artifact list's shape, not its contents against
   `report_versions` in the control copy. Replacing it with `[]` still verified.
3. Retention accepts any `b1-` alphanumeric name, and treats an unreadable or corrupt
   manifest as an interrupted run. With `keep=3` it deleted an unrelated `b1-000-…`
   directory and a verified set whose manifest had become unreadable.
4. The deployable validator requires only the service image mapping. With the `kubectl`
   entry removed, it printed `DEPLOYABLE` while the orchestrator ran the image `kubectl`.
5. Checklist step 12 still runs the low-space proof in the quarantined drill, not the
   separate `airs-mcp-qual` deployment.

Minor findings:
- The daily-backup recovery timing is overclaimed, and the stop wait, the Job timeout
  and the orchestrator deadline are misaligned. Scaling to 1 does not release an RWOP
  volume the Job still holds, and the success marker precedes the restore trap.
- DESIGN §22.8 still exempts all lifecycle operations, and CONTRACT §9 still describes
  hidden token input. The retention and latest-selection options are undocumented.
- The capacity accounting budgets one backup set, not three retained sets plus one
  being created.
- `--keep` with no value, or given twice, is treated as absent instead of as a usage
  error.

### M8 round 4 corrections (submitted for source-side review 4)

1. **The derive helper is offline under every option.** `--run-lookups` is removed and
   now exits 2 ("offline only"). For each value the helper prints the read-only commands
   for the owner to run, then takes the pasted, screened result or the suggestion. It
   reads only local files, and the only program it runs is the repository's own
   `airs-mcp check-config` on its output.
   - Tests: `derive_config_runs_no_service_cli_under_any_option` (failing stubs for az,
     kubectl, cf, airs, mise, curl and wget; default mode, `--help` and `--run-lookups`)
     and `the_derive_helper_source_invokes_no_service_cli` (static, with comments,
     quoted text and heredocs stripped).
   - Mutation: an injected `az account show` call fails the static test and three
     runtime tests.
2. **Verify compares the artifact inventory with the control copy.** `control_artifacts`
   is the one derivation used by creation and verification. Verify rebuilds the
   inventory from the verified control copy's `report_versions` and requires exact,
   order-insensitive equality of IDs, hashes, bytes and count.
   - Tests (`maintenance.rs`): `[]`, a changed hash, changed bytes, a duplicate and an
     extra entry are each refused.
3. **Retention fails closed.**
   - `is_set_name` accepts only `b1-YYYYMMDDTHHMMSSmmmZ-<8 lowercase hex>`, the exact
     grammar `create` produces.
   - A set's manifest is Complete, Missing (an interrupted run, removable when older than
     the newest), Incomplete, or Uncertain (unreadable, not JSON, not a file, or a foreign
     format). Any Uncertain set makes `prune` remove nothing and return an error, and
     `latest_complete` refuses when an Uncertain set is newer than the newest complete
     one. Deletions are collected first and performed only after every check passes.
   - Test: `retention_fails_closed_on_uncertain_sets_and_ignores_foreign_names`
     (`b1-000-not-a-backup` is never touched; a corrupt, unreadable or foreign manifest
     blocks retention; after repair only the interrupted run is removed).
4. **Every image is validated.** `check_images` walks every container and init-container
   image of every resource in the kustomization. Each is a bare name with exactly one
   mapping, or a fully qualified `@sha256:` reference. The mappings must exactly cover the
   bare names (no missing, extra or duplicate mapping), with a registry `newName` and a
   well-formed digest; deployability refuses placeholders.
   - Tests: the pin test adds five mutations on a fully pinned copy (kubectl mapping
     removed or renamed, an unused mapping, an unmapped init-container image, an
     init-container image pinned by tag only).
5. **Checklist step 12 follows the `airs-mcp-qual` deployment** of runbook §12 exactly:
   isolation with no AIRS egress, the authenticated temporary route, seed or restore with
   deliberate reconciliation and schedules disabled, fill, observe, free, resume and
   clean up. It says never to use the quarantined drill namespace.

Minor findings:
- *Backup timing.* The orchestrator polls the Job (a failure exits at once), its exit trap
  waits for the set Job's pod to be gone (the RWOP volume released) before scaling to 1
  and then polls `availableReplicas`, and a TERM/INT trap makes the exit trap run on
  termination. It logs `backup_set_verified`, `service_restored` and
  `backup_set_daily_complete` separately and exits 0 only when both succeed. Deadlines
  nest: set Job 1200 s < poll 1230 s ≤ pod wait 1260 s; stop 300 + pod 1260 + ready 300
  + 300 margin ≤ orchestrator 2400 s. The README drops the "back by 03:14" claim and
  documents recovery after an interrupted orchestrator. `check_backup` checks the step
  order and deadline nesting statically, and
  `the_backup_orchestrator_restores_the_service_and_reports_honestly` runs the real
  script against stub `kubectl`/`sleep`/`date` for success, a failed set, and a service
  that never becomes available.
- *Documentation.* DESIGN §22.8 describes the lifecycle classification, the fail-closed
  measurement cases, `--keep N`, `--latest-in DIR` and verify's inventory and metadata
  checks. CONTRACT §9 replaces the hidden-token text with the offline inventory
  workflow (exit codes 3 and 4).
- *Capacity.* Runbook §12 accounts separately for artifacts, the live databases, three
  retained sets and the creation-time peak (a fourth set before pruning): about 7×D + A
  + 20%, and the sets must fit outside the disk reserve.
- *`--keep` parsing.* `strict_flags` runs before any configuration or storage step on
  every maintenance command and on `control-backup-verify`: only known `--flag value`
  pairs, each at most once, with no value starting `--`; otherwise exit 2. Ten cases are
  tested, including a missing value, a duplicate, `0`, two `--set`, and `--set` with
  `--latest-in`.

Each mutation was checked alone and caught: fork S's `az` injection; fork R's Q1
(inventory comparison removed), Q2 (lenient names), Q3 (Uncertain sets do not block),
Q4 (unmapped images accepted), Q5 (unused mappings accepted), Q6 (`strict_flags` off),
Q7 (no wait for the set pod before scale-up) and Q8 (success marker without restoration).

### M8 source-side review 4 — 8.0/10, revise (preserved, 2026-11-07T12:24Z)

Source: `.local/dev/runtime-mcp/m8/review-4/result.md`. All 712 frozen files verified.
Most review-3 probes pass.

Material findings:
1. The backup orchestrator waits for the set Job's pod to be deleted, but a completed
   pod stays (the Job has a 14-day TTL), so the real script exits 1 without restoring
   the service. The stub test makes every wait succeed. The sequential bounds (300 +
   1,230 + 1,260 + 300 = 3,090 s) exceed the 2,400 s deadline, and the exit trap has no
   matching termination grace period.
2. Retention treats a recognized-format manifest whose `complete` is missing or not
   `true` as an interrupted run. Removing only `complete` from a verified set's manifest
   got it deleted under `keep=3`.
3. The qualification route (runbook §12) adds a second gateway integration against the
   production upstream audience and client, which §7a forbids (one confidential client
   per integration).
4. The low-space filler asks UID 65532 to create `/data/filler`, but `/data` is
   root-owned 0755 (contract §9.3), so the command cannot run.

Minor finding: a literal image reference with an all-zero digest is accepted as
deployable; only mapped digests were checked for the placeholder.

### M8 round 5 corrections (submitted for source-side review 5)

1. **The orchestrator releases the volume by pod phase, within its deadline.**
   - The exit trap no longer waits for the set Job's pod to be deleted (a completed pod
     is retained for 14 days). It polls pod phases and proceeds once every set-Job pod is
     `Succeeded` or `Failed`, or none exists; a terminal pod holds no RWOP attachment.
     No delete permission was added.
   - The Job poll reads `status.conditions`: `Complete=True` succeeds, `Failed=True`
     exits 1 at once.
   - Every `kubectl` call carries `--request-timeout=30s`, and the trap disarms itself
     against re-entry.
   - Budgets: set Job 1200 s < poll 1230 s ≤ release wait 1260 s; the full path
     300 + 1230 + 1260 + 300 + 300 margin = 3390 s < `activeDeadlineSeconds` 3600;
     `terminationGracePeriodSeconds` 1980 = release 1260 + ready 300 + 4 × 30 s API
     calls + 300 margin.
   - `check_backup` asserts the step order, the absence of a delete-wait, the condition
     polling, the request timeouts, both budget inequalities and the grace period.
   - `the_backup_orchestrator_restores_the_service_and_reports_honestly` runs the real
     script against stateful stubs: a completed Job with a retained `Succeeded` pod
     (exit 0, all three events); a failed Job (restored, exit 1, no completion event);
     SIGTERM mid-poll (the trap restores, exit 143); a Job that never completes (the
     poll runs to its bound, restored, exit 1); a pod that never terminates (no
     restoration claimed, exit 1); and a service that never becomes available.
   - The README and runbook §9a carry the same numbers and the worst-case downtime.
2. **Retention fails closed on any non-complete manifest.** Creation only writes
   `complete: true`, so `Missing` (no manifest) is the only interrupted-run state. Any
   present manifest that is not exactly a complete one of this format is `Uncertain`:
   unreadable, not JSON, a foreign format, a missing `created_at`, `epoch`, `stores` or
   `artifacts`, or `complete` missing, `null`, `false`, `"true"` or `1`. `prune` then
   refuses and removes nothing, and `latest_complete` refuses when such a set is newer
   than the newest complete one. Each case is a regression.
3. **The qualification route has its own identities.** Runbook §12 provisions a separate
   Entra API registration (its own App ID URI and scopes) and a confidential-only
   gateway client, derives the qualification config from production (hostname,
   `audiences`, `allowed_client_ids`), repeats §7a's four binding checks with the
   qualification IDs before any sign-in, and removes every temporary registration,
   integration and DNS entry in cleanup, re-checking §7a against production afterwards.
   §7a states that the production client never serves a second integration. Checklist
   step 12 matches.
4. **The filler is a 65532-owned sibling.** The qualification overlay's init container
   (root, `CAP_CHOWN` only) creates `/data/qual-filler` 0700 owned by 65532 beside
   `/data/airs`; ancestors are unchanged, and production manifests never create it. The
   choice follows the code: `storage::admit` checks the target and its ancestors only,
   and the reserve sums the control and history stores only, so a sibling is neither
   refused nor counted. Fill and free steps run as 65532 through a debug pod or
   ephemeral container, since the service image has no shell.
   - Test: `the_qualification_filler_is_a_sibling_the_reserve_never_counts`.

Minor finding: `check_images` rejects the all-zero placeholder in literal `@sha256:`
references as well as in mappings; the pin test adds an init container pinned to 64
zeros.

Each mutation was checked alone and caught: fork R2's R1 (non-boolean `complete` as
interrupted), R2 (literal placeholder accepted), R3 (wait-for-delete restored), R4
(deadline 2400), R5 (grace 300), R6 (no request timeout), R7 (no TERM trap); fork S2's M1
(the reserve counting siblings) and M2 (admission refusing a data directory with a
sibling).

### M8 source-side review 5 — 8.5/10, revise (preserved, 2026-11-07T13:04Z)

Source: `.local/dev/runtime-mcp/m8/review-5/result.md`. All 712 frozen files verified.
Every review-4 probe passes except the two below; the retention regression was shown to
fail on the review-4 source and pass on round 5.

Material findings:
1. The orchestrator counts only its sleeps (5 s per iteration), not the time `kubectl`
   calls take, and the static budget check assumes four request costs during termination
   although the release loop allows 252 requests. With release queries each failing after
   29 s (inside the 30 s request timeout), a SIGTERM path requested the scale-up 8,563 s
   later, far past the 1,980 s grace period. The stub tests pass because their API
   responds at once and their sleeps take no time.
2. Runbook §12's credential step runs `az ad app credential reset … --query keyId`, but
   the command returns `appId`, `password` and `tenant`, never `keyId`, and selecting an
   identifier cannot preserve the one-time credential value the next step needs.

Minor finding: the filler recipe uses GNU `df --output=avail` in a BusyBox helper, which
rejects it, leaving the allocation arithmetic with an empty value.

### M8 round 6 corrections (submitted for source-side review 6)

1. **The orchestrator keeps elapsed-time deadlines.** It remains POSIX `/bin/sh` with no
   new dependency (`now() { date +%s; }`).
   - Constants `REQ=30 STOP=300 POLL=1260 RELEASE=1260 RESTORE=360 MARGIN=300`.
   - One `kubectl` call site, `kc DEADLINE …`, whose request timeout is the smaller of
     `REQ` and the time left to the phase deadline; it issues no request once none is left
     (exit 124). Every phase (stop wait, Job poll, release wait, ready wait) is a fresh
     wall-clock deadline; no loop counts iterations.
   - The trap keeps its own clock, starts `RESTORE` afresh after the release wait, so an
     exhausted release wait cannot consume it, and ignores a further TERM so the grace
     period alone bounds it. Budget exhaustion in the stop wait is never read as "pods
     gone".
   - `activeDeadlineSeconds` is 3900 (was 3600); `terminationGracePeriodSeconds` stays
     1980. Full path 30 + 300 + 30 + 1260 + 1260 + 360 + 300 = 3540 < 3900; termination
     path 30 + 1260 + 360 + 300 = 1950 ≤ 1980.
   - `check_backup` parses the constants from the script and the deadlines from the
     manifests and asserts both inequalities, `tmpl < POLL`, `tmpl + 60 ≤ RELEASE`,
     `RESTORE ≥ 2·REQ + 300`, the single `kubectl` inside `kc`, the absence of iteration
     counters, each phase's deadline guard, each request bounded by its phase, and the
     fresh `RESTORE`.
   - The scenario test runs the real script under `/bin/sh` with stub `date`, `sleep` and
     `kubectl` on a virtual clock (`sleep N` advances N; a request advances the smaller of
     its cost and its timeout). Cases: a completed Job with a retained pod; a failed Job;
     SIGTERM after a poll; a Job never terminal; a pod never terminal; a service never
     available; SIGTERM followed by 29 s failing release queries (scale-up within
     REQ + RELEASE, the trap ends within grace − margin); 29 s polls never terminal plus
     slow release (the whole run within deadline − margin); and SIGTERM at the start of a
     29 s request. Every case checks each timeout ≤ `REQ` and ≤ the time left in its phase.
   - The README and runbook §9a carry the new numbers (65 min deadline, 33 min grace, the
     3540 s / 1950 s worst cases); no 1230, 3390 or 3600 remains.
2. **The qualification credential is an owner-only handoff.** The wrong `--query keyId`
   line is gone. In the owner's terminal, with history off, `az ad app credential reset
   … --query password -o tsv` is pasted directly into the gateway console's secret for
   the integration, the terminal is cleared, and only `az ad app credential list …
   --query "[].{keyId, displayName, end}"` is recorded. The evidence file holds only the
   key ID and expiry. The integration reaches the secret by name (`--secret-mappings`),
   and `--configurations` takes inline JSON only, so it never carries the value. The
   installed `az` source confirms `credential reset` returns `appId`, `password` and
   `tenant`, never `keyId`.

Minor finding: the filler recipe uses `df -Pk … | awk 'NR==2{print $4}'`, refuses a
non-numeric or zero result before any arithmetic, and keeps `fallocate` with a `dd`
fallback. It was exercised inside the pinned `busybox:1.37.0` image: `df --output` is
rejected there, the POSIX pipeline works, and `fallocate`, `dd`, `stat -c` and `rm` are
present.

Each mutation was checked alone and caught: m1 iteration counting (scale-up 2340 s after
SIGTERM against a 1290 s bound), m2 no per-request cap (a 30 s request overruns its
deadline), m3 the ready wait reusing the release deadline (no scale-up requested when
the pod never terminates).

Also in round 6, found by PR #10's CI: the CI image is Ubuntu 22.04 with Python 3.10,
which has no `tomllib`, so `derive-config.sh`'s validation failed there. The helper now
probes for `tomllib` or `tomli`; without either, suggestions from the M0.6 file are
absent, the grant tools fall back to the built-in proof-ready lists, the Python
round-trip is skipped, and `airs-mcp check-config` becomes mandatory (no binary and no
parser refuses to write). Exercised in the CI image: the script completes with a
`check-config` stub, and refuses with neither validator.

### M8 source-side review 6 — 8.7/10, revise (preserved, 2026-11-07T13:52Z)

Source: `.local/dev/runtime-mcp/m8/review-6/result.md`. All 712 frozen files verified.
The round-5 probes pass: under virtual time the SIGTERM path ends after 1,270 s (round 5:
8,568 s) and the slow-poll path after 2,530 s (round 5: 16,922 s). The credential
handoff and the BusyBox recipe are correct.

Material findings:
1. `kubectl --request-timeout` bounds one server request, not the invocation; discovery
   can issue several (five were observed in one credential-free attempt). Modelling five
   requests per invocation, the termination path reached 2,005 s against the 1,980 s
   grace period. The regression caps the whole simulated invocation at one request
   timeout, so it cannot see this.
2. `derive-config.sh` keeps a non-empty `AIRS_MCP_BIN` even when the path is missing or
   not executable; the mandatory-validator guard only tests emptiness. With no Python
   TOML parser, a missing or non-executable validator path led to an unvalidated file
   being written (exit 0).

Minor finding: an older ledger summary still describes the R2 helper as inspecting pasted
tokens through API calls.

### M8 live evidence (owner-run with the agent, 2026-11-07): findings and fixes

Steps 1–3 and 6–9 of the runbook were performed against the owner's cluster (GitOps:
talos-cluster `airs-mcp/deploy` overlay on this repository's `deploy/airs-mcp` at a pinned
commit; Argo-managed; Harbor image `registry.cdot.io/airs-mcp/airs-mcp@sha256:c1c9888c…`).
The service came up as designed (uid 65532, encrypted RWOP volume attached once, ESO
secrets, Let's Encrypt TLS, `storage_owned` → `control_recovered` → `listening`, probes and
metrics clean) and answered the CLI and Claude Code through the gateway. Four findings:

1. **Owner decision — environment-specific backup gates withdrawn.** Contract §9.4's
   "open checks" (a second NAS's retention record; no agent-accessible R2 delete right)
   described the reference environment's backup chain, not the product, which stores
   everything in SQLite on its own volume. Rewritten as operator-owned policy; runbook
   steps 4–5 are optional operator records (PR #14).
2. **Grant binding is exact, as designed.** Claude Code's dynamic registration had changed
   since M0.6 (`pk_66d09ded…` → `pk_610f537e…`); the reader grant refused the new client
   (`caller and agent have no grant`) until the configuration named it (talos-cluster
   PR #454, revision prod-20261107-2). The login helper note: the gateway registers the
   CLI client with an exact loopback redirect, so `airs mcp login` needs `--port 53682`
   (runbook §7b, PR #13).
3. **Collector defect (fixed): dashboard rolling window.** The first collection left the
   `inventory` and `sessions` sources `unavailable`. The SCM dashboard API accepts rolling
   windows of 24 hours or 1, 7, 30 or 60 days only; the service derived its window as
   `ceil((now − target.start) / 1 h)` at run time, which for a 24 h target submitted seconds
   earlier is 25 hours — refused with HTTP 400 on every ordinary collection. Fix:
   `airs-history-store::sync::dashboard_window` rounds the span up to the smallest accepted
   shape (24 h; 7, 30, 60 days; the exact count is kept past 60 days so the refusal stays
   visible), with a unit test; the applications and sessions queries then succeed (direct
   probes: 24 h, 7 d and 30 d accepted; 25 h, 48 h and 2 d refused). Two refinements followed
   from the redeployed service: (a) `inventory` stayed unavailable because the category
   reported the worst of its sources and `application-telemetry` is unavailable by design
   for any window but 7/30/60 days — it is no longer coverage (contract §4.3; still a Signal
   Brief source); (b) a 24 h target derived a moment later (25 whole hours) stays the
   24-hour window rather than 7 days, because the polling lower bound returns to the first
   collection's 24 h while any source lacks a complete traversal, and a page-capped 7-day
   sessions read would never complete on a larger tenant (contract §6). **Recollection
   receipt** on the redeployed image (`sha-27e9654`, sha256:fc830c2b…, 2026-11-07T22:57Z):
   `collection_start` succeeded (4 management sends, 0 unfinished) and
   `history coverage` for 22:45–22:57Z, a range only that run covers, shows inventory,
   profiles, results and sessions all **complete** with gap 0 (the first run on the
   reviewed image had inventory and sessions unavailable); repeated on the final image
   (`sha-e3060b7`, sha256:7c81056d…, 2026-11-07T23:06Z): job succeeded, 4 management
   sends, all four sources complete, gap 0, for 22:58–23:06Z. These are source changes
   after review 8 and are part of the final combined review's scope.
4. **Orchestrator defect (fixed): its `kubectl` could not reach the API server.** The
   first by-hand daily set run (step 9a) failed before scaling anything: `kubectl` in the
   orchestrator pod dialled `localhost:8080`. Two causes, each confirmed by a labelled
   probe pod in the same namespace: (a) with kubectl 1.37 the `--request-timeout` flag
   made the client ignore the in-cluster configuration, so the `kc` wrapper now bounds
   each call with `timeout` alone and the static test asserts the flag's absence;
   (b) under Cilium's kube-proxy replacement without socket-level load balancing, the
   base policy's egress to the API server's node addresses did not match the
   Service-translated backends (node addresses carry the remote-node identity), so the
   API-server endpoints are an overlay value: the public base ships a documentation
   address (`192.0.2.1/32`) that the deployability check refuses, and the talos overlay
   sets the control-plane VIP and node addresses on port 6443 and points
   `KUBERNETES_SERVICE_HOST`/`_PORT` at the VIP (talos-cluster PR #461; runbook §6;
   PR #18). Argo had also healed the orchestrator's scale-down back to one replica
   before the set could run; the Application now ignores the Deployment's replica count
   (talos-cluster PR #460; runbook §6, PR #17).

The marker report (step 9b) was created on real data and its positive control passed
(`grep -c` of the marker in the downloaded `report.html` = 1).

**Step 9a receipt** (2026-11-08T00:12Z, base 1397935 with the overlay of talos-cluster
PR #461): the by-hand daily job completed in 50 s — `backup_set_verified` for set
`airs-mcp-backup-set-20261108001225` (the set job logged
`{"artifacts":3,"epoch":"62d2f6ef","event":"backup_set_verified","stores":2}`),
`service_restored`, `backup_set_daily_complete`; the service was gone for about 40 s and
came back with a new owner id (`storage_owned` → `control_recovered` → `listening`).

**Steps 9b.5–9b.6 and 11 receipts** (2026-11-08T00:13–00:30Z): a Longhorn backup of the
encrypted volume taken after that set (`airs-mcp-drill-20261108001348`, lz4, 2.43 GB)
was restored twice into the isolated `airs-mcp-drill` namespace (default-deny network,
placeholder credential files, a throwaway TLS Secret, Deployment at 0; talos-cluster
PR #459). Through a class whose passphrase Secret holds a wrong value, the device
attached but never mounted: nine `MountDevice` failures, `cryptsetup luksOpen … exit
status 2`, pod never Running. Through the correct class: the carried set verified
read-only (the same `backup_set_verified` line); `control-backup` (188 416 B, epoch
62d2f6ef) → store moved aside → `control-restore` (`control_restored`, pending epoch
bbc00ba5) → `control-activate-epoch` (`control_epoch_activated`); `control-reconcile`
not run. Started at one replica, the copy logged `storage_owned`, `control_recovered`
(0 requeued, 0 uncertain) and `listening`; `/readyz` ready, `airs_mcp_control_ok 1`,
`airs_mcp_archive_bytes 1077248`, `airs_mcp_control_bytes 188516` and the job-state
series (4 succeeded, 3 partial) identical to production's at the time of the backup.
Scaled back to 0, the marker title was found in the restored control store, the
preserved store and the bundle's `report.html`; a negative-control string was not.
`history_status` against the drill copy was not made (no route). The namespace, both
drill classes, the wrong-key Secret and the drill snapshot were deleted; the backup
stays on the target, unmodified.

**Step 9b.4 receipt** (2026-11-08T00:44Z): the volume is `encrypted: true` on a class
that references the passphrase Secret; a read-only pod on the backup target decompressed
every block the backup lists (1,159 blocks, 2,430,599,168 bytes, lz4): marker title 0
occurrences (it occurs twice in the plaintext control store and in `report.html`), no
`SQLite format 3` header, none of the volume's file names, the LUKS magic exactly once
and at the volume start, median block entropy 7.9999 bits/byte. 142 blocks fall below
7.9 bits/byte (the LUKS header area and 2 MiB blocks only partly written — dm-crypt
leaves never-written sectors as zeros), which puts the overall figure at 7.76; nothing
structured besides the LUKS header was found.

Not performed in this run: step 7a's gateway-console item 4; step 10 (needs disposable
worker nodes and hypervisor power control); step 12 (needs its own Entra registrations
with an owner-only secret handoff and a gateway integration). The checklist marks them.
The final combined review remains.

### M8 source-side review 8 (preserved; 2026-11-07T16:03Z, Codex, frozen tree a02899d7, receipts m8/run9)

**9.0/10, accept (source side).** "The round-7 termination defect and both minors are
fixed. The source, artifacts, checks and procedures are sufficient for the owner to
produce the remaining integration evidence. This does not accept the deployment or
complete overall M8." Material findings: none outstanding. Minor findings: none
outstanding. 709 files matched the manifest before and after; no external services,
credentials, images or cluster resources accessed.

Independent virtual-time probes: SIGTERM then slow failing release queries, scale-up and
exit at 1,265 s; slow nonterminal polls and releases, whole run 2,520 s; SIGTERM at the
start of a 29 s request, restoration at 29 s; five-request release invocations each cut
at REQ, exit at 1,265 s; hung first poll killed after 35 s, polling continues, run ends
at 1,260 s; hung first release query killed after 35 s, next query succeeds, restoration
45 s after SIGTERM; **combined SIGTERM + hung poll + failing multi-request releases: trap
starts at 35 s, scale-up and exit at 1,295 s** (within 1,300 and grace − margin 1,680;
exit 1, no false restoration or completion event). A real `/bin/sh` + GNU `timeout`
probe at a 20× clock: trap entry 1.755 s, scale-up 64.788 s, exit 64.810 s real (≈35.09,
1,295.76, 1,296.20 virtual s). Arithmetic matches the code: 3,570 < 3,900;
(30+5) + (1260+5) + (360+5) + 300 = 1,965 ≤ 1,980. Mutations discriminate: `$r` bound
(static check and executable bound assertions; combined case reaches scale-up at
2,525 s), `sleep 5` for `nap` (static and phase-sleep assertion), unconditional 5 s
`nap` (next phase at 1,270 against 1,265), no `-k`, no wrapper, grace 1,960.

Remaining scope judged sound within source-review limits (manifests with 32 security
mutations and pin checks passing; metrics; helpers; the honestly-scoped tmpfs
qualification; release/runbook; no M2–M7 regression; CLI dependency graph excludes the
server, history store and SQLite). Receipt: 4,243 passed, 0 failed, 9 ignored, no
parity skips; the reviewer's own service tests were socket/storage-blocked; dash
execution rests on the disclosed CI evidence.

**Overall M8 acceptance still requires the owner's live evidence**: image/platform
provenance; actual hardening, secret readability, TLS and network isolation; dedicated
gateway binding and CLI/Claude operation; nas1 retention and the complete R2-access
inventory; a verified coordinated backup followed by off-volume capture; positive
encryption evidence, wrong-key refusal and correct-key marker recovery; fenced
disposable-node reacquisition without overlapping writers; isolated restore and epoch
handling; production-storage low-space recovery, capacity and coordinated-backup memory
measurements (`scripts/m8/user-checklist.sh`, runbook steps 1–12).

### M8 round 8 corrections (reviewed by source-side review 8: 9.0, accept)

1. **The wrapper's bound is the lesser of `REQ` and the time left, never the whole
   phase.** `kc` runs `timeout -k "$KGRACE" "$t" kubectl … --request-timeout="${t}s"`
   with `t = min(REQ, d − now)`. No invocation, hung or not, lasts longer than
   `REQ + KGRACE` = 35 s, so that is also the longest the shell can defer the TERM trap
   while a foreground `kubectl` runs; the termination arithmetic's first term (35 s) now
   describes the code. An invocation cut at its phase deadline still overruns it by at
   most `KGRACE`, and polling or releasing continues after a hung invocation instead of
   losing the phase to it.
   - **Loop sleeps are bounded by the phase deadline.** `nap DEADLINE` sleeps the lesser
     of 5 s and the time left (nothing once the deadline has passed) and replaces every
     `sleep 5` in the four loops, so a phase overruns by at most `KGRACE`, from one cut
     invocation, never from a sleep after it (review 7 minor 1). Budgets are unchanged
     (full path 3570 < 3900; termination (30+5) + (1260+5) + (360+5) + 300 = 1965 ≤ 1980)
     and their assumptions now hold; README and runbook §9a state the new guarantee.
   - Static checks: the `kc` string requires the `"$t"` bound; `nap` must be defined as
     above, no bare `sleep 5` may remain, and each loop must end with `nap "$dX"`.
   - Harness: `bounded()` additionally asserts every invocation's bound ≤ `REQ`, and that
     whatever follows a phase starts no later than max(deadline, the phase's last
     invocation end) + 1 s (a sleep never outran the phase). Scenario (i) now asserts
     every five-request release invocation is cut at `REQ` (TERM answered at the bound);
     (j) a hung first poll is KILLed 35 s in and polling continues to the budget; (k) a
     hung first release query is KILLed 35 s in, the next query sees the terminal pod and
     the service is restored within two bounded invocations of SIGTERM (exit 143,
     `service_restored`, no completion). **New (l)** (review 7's counterexample): SIGTERM
     at the start of a TERM-ignoring hung Job poll, then five-request failing 29 s
     release queries: the trap starts within `REQ + KGRACE` of SIGTERM, the scale-up
     within (30+5) + 1260 + 5, the trap ends within grace − margin, exit 1 and no
     `service_restored` (the release wait failed honestly).
   - Mutations (each alone, against the committed file the scenario test reads; the
     static check was relaxed where noted so the scenario had to catch it): bound back to
     `$r` → the static `kc` check fails; with that relaxed, (i) fails (release invocations
     not cut at REQ); with (i)'s cut assertions relaxed too, (j) fails ("the hung poll is
     bounded by REQ"); bare `sleep 5` → the static `nap` check fails; `nap` made an
     unbounded 5 s with the static check relaxed → the phase-sleep assertion fails
     ("job-name=: the next phase started at …1270, after the deadline …1265 and the last
     invocation's end …1265: a sleep outran the phase").
2. `scripts/m8/build-hashes.sh` header: the x86_64 host build is recorded for the CLI
   release and never compared with the image's binary (review 7 minor 2), matching
   `docs/RUNTIME_MCP_RELEASE.md` step 2.

### M8 source-side review 7 (preserved; 2026-11-07, Codex, frozen tree a6e93205, receipts m8/run8)

**8.8/10, revise.** Verified: round-6 finding 2 (derive validator matrix: unset, missing,
non-executable, rejecting, accepting; the round-6 code made the regression fail) and the
R2 summary minor are fixed; the whole-invocation `timeout` works and its regressions
discriminate (no wrapper: 1,355 > 1,295; no `-k`: 1,280 vs 1,265; grace 1,960 fails the
static check); a real GNU `timeout` ended a TERM-ignoring child and its descendant in
0.401 s; the `timeout` preflight exits 1 before any trap or call; static constants
3,570 < 3,900 and 1,965 ≤ 1,980. Receipts: 4,243 passed, 0 failed, 9 ignored, no
parity skips; the reviewer's own run was socket/storage-blocked for the service tests.

Material finding (must fix): **SIGTERM during a hung poll can exceed the grace before
scale-up.** `kc` gave `timeout` the whole remaining phase budget `$r`, so a first Job
poll could run 1,260 s, while the termination arithmetic charges the in-flight
invocation only REQ + KGRACE = 35 s. The shell defers the TERM trap while a foreground
child runs, so SIGTERM at the start of a TERM-ignoring poll followed by five-request
failing release queries reached scale-up at 2,530 virtual seconds, past the 1,980 s
grace (reproduced with the real shell and GNU `timeout`). Fix: bound each whole
invocation at min(REQ, remaining phase time) (the existing `$t`), keep `-k KGRACE`, add
the combined scenario, account for loop sleeps, update the static arithmetic, README
and runbook together.

Minor: the runbook's "a phase overruns by at most five seconds" ignores the `sleep 5`
after a cut invocation (hung-poll scenario: 1,270 s against POLL=1260); bound the sleep
by the remaining time or document it. `build-hashes.sh:17` still says the host binary is
compared with the container binary; align it with the release procedure.

### M8 round 7 corrections (reviewed by source-side review 7: 8.8, revise)

1. **Every `kubectl` invocation is bounded by elapsed time, not only each request.** `kc`
   is now `timeout -k "$KGRACE" "$r" kubectl -n "$NS" --request-timeout="${t}s" …`, with
   `r` the time left until the phase deadline and `t` the lesser of `REQ` and `r`
   (`KGRACE=5`). Coreutils `timeout` sends TERM at the bound and KILL `KGRACE` later, so an
   invocation that issues several server requests, or a child that ignores its request
   timeout, overruns its phase by at most `KGRACE`. Each phase's budget charges that:
   full path (30+5)+(300+5)+(30+5)+(1260+5)+(1260+5)+(360+5)+300 = 3570 < 3900;
   termination path (30+5)+(1260+5)+(360+5)+300 = 1965 ≤ 1980. README and runbook §9a
   state the same arithmetic and the new guarantee.
   - Found while proving it: the trap's `trap '' TERM INT` *ignored* TERM, and an ignored
     signal is inherited across `exec`, so every `kubectl` the trap launched ignored
     `timeout`'s TERM and each cut cost the full `KGRACE`. The trap now installs a no-op
     handler, `trap ':' TERM INT`: a second TERM still does not end the trap, but its
     children exec with default TERM disposition. The static check asserts the handler
     and the absence of `trap ''`.
   - Preflight: the bound depends on `timeout` existing in the pinned kubectl image, so
     the script checks `command -v timeout` before it sets any trap or makes any call,
     and otherwise logs `{"event":"orchestrator_preflight_failed","missing":"timeout"}`
     and exits 1 with the service untouched. The static check requires the preflight to
     precede the first `trap`; runbook step 2 tells the owner to confirm the pinned
     image has coreutils `timeout` (`podman run --rm --entrypoint timeout <image> --version`).
   - Harness (`deploy_manifests.rs`, orchestrator scenario test): a virtual-clock
     `timeout` stub (TERM at the bound, KILL at bound+k, exit 124/137; a child alive 20 s
     past its bound is logged `RUNAWAY` and fails the test instead of hanging); the
     `kubectl` stub spends time in 1 s steps so cuts land mid-request, supports N
     requests per invocation and TERM-ignoring hangs; log lines record start, finish or
     cut time and the bound; clock writes are atomic. `bounded()` asserts per invocation:
     request timeout ≤ bound ≤ remaining phase time, finished within its bound, cut within
     bound+`KGRACE`, no runaway, phase overrun ≤ `KGRACE`. New scenarios: (i) five 29 s
     failing requests per release invocation on the termination path (exactly one cut, at
     the release deadline; scale-up ≤ REQ+RELEASE+KGRACE after SIGTERM; trap within
     grace−margin); (j) the first Job poll hangs ignoring TERM and its request timeout
     (KILLed at 1265 s, the poll gives up, the trap restores the service, run ≤
     3900−300); (k) the first release query in the trap hangs (KILLed at 1265 s, release
     wait fails honestly: no `service_restored`, exit 1, service still scaled up within
     1295 s of SIGTERM, trap within grace−margin). Scenarios a–h unchanged.
   - Mutations (each alone): removing the `timeout -k` wrapper fails the scenario test at
     the first invocation and the static check; with the bound-presence checks relaxed,
     scenario (i) reproduces the review's overrun ("scale-up requested 1355 s after
     SIGTERM, bound 1295"); removing `-k "$KGRACE"` fails (j) ("the hung query lasted
     1280 s: bound 1260 then KILL after 5") and the static check; grace 1980→1960 fails
     the static arithmetic (1965 ≤ 1960).
   - Live proof still owner-run: `timeout` present in the pinned image and a real kubectl
     answering TERM; a kubelet TERM reaching PID 1 and the trap finishing within the
     1980 s grace; one observed daily run (runbook §9a, checklist step 9).

2. **The derive helper publishes only after a validator ran.** The validator is resolved
   into its own variable as the first candidate that is a regular, executable file. An
   `AIRS_MCP_BIN` that is set but not such a file is a hard error ("not an executable
   file; fix it or unset it"), never silently replaced by a discovered binary. Without a
   Python TOML parser and without a validator the helper refuses to write (exit 1).
   `AIRS_DERIVE_ASSUME_NO_TOML=1` forces the no-parser path for tests; it can only make
   validation stricter.
   - Test: `derive_config_requires_a_working_validator_without_a_toml_parser`: unset
     (the helper may discover the built binary; a file exists only if `check-config`
     accepted it), missing path and non-executable file (exit 1, no file, the error
     named), a rejecting validator (exit 1, no file) and an accepting one (exit 0, file).
   - Mutation: the review-6 discovery loop fails the missing-path case.

Minor finding: the M8 implementation summary now describes the R2 helper as the offline
inventory validator it became in round 2, with the original token-inspection design
noted as replaced.

CI correctness (not a reviewer finding): PR #10's `linux` job failed on
`the_backup_orchestrator_restores_the_service_and_reports_honestly`. CI's `/bin/sh` is
dash, and the test's `kubectl` stub logged each call with `echo`, which expanded the
literal `\n` in the orchestrator's jsonpath argument into a newline and split the log
line. The stub logs with `printf` now. Reproduced and verified in the CI image
(`catthehacker/ubuntu:act-22.04`, unprivileged user, `RUST_TEST_THREADS=4`): the ten
`deploy_manifests` tests pass under dash. The orchestrator script itself already used
`printf` for data that can carry backslashes.

