# Runtime MCP M0 feasibility record (M0.2–M0.4 loopback evidence)

Status: executed 2026-11-04 (round 1) and 2026-11-05 UTC (round 2, after independent
review 1) on the enterprise host (Linux amd64). This is **loopback
and package evidence only**. It cannot pass the M0.6 live gate
([runbook](RUNTIME_MCP_M0_6_RUNBOOK.md)). Receipts:
`.local/dev/runtime-mcp/m0/` (logs, feature diffs, size JSON, mutation log).

## 1. Prototype workspace

`prototypes/runtime-mcp/` is an isolated Cargo workspace, like the B0 SQLite
prototype. The shipped `airs` binary does not link it, and it is not a root
workspace member. Members:

| Crate | What it proves |
| --- | --- |
| `airs-runtime-core` (skeleton, approved name) | Registry-resolvable `scm-sdk =0.2.4`; the frozen per-tool schema/example fixtures and their validator test; strict versioned DTO envelope; America/Chicago day bounds and revision/allowance rules on pinned jiff; `HistorySink` + nonpersisting sink. |
| `airs-mcp-client` (skeleton, approved name) | rmcp Streamable HTTP client on the pinned reqwest 0.13.5 with no SQLite; 256 KiB complete-result bound. |
| `airs-mcp-spike` | The M0.6 protected echo/info backend. It does two-token verification on every request; enforces grants, tiers and allowed-tool lists; serves RFC 9728 metadata, bounded pages and chunks; gates admin execution behind the fresh proof; handles TLS with reload; enforces storage admission and single-owner locking; runs the synthetic SQLite backup drill. |
| `airs-mcp-spike-cli` | CLI-side OAuth: approved-origin/HTTPS URL trust, PKCE through rmcp, loopback callback, `state`/`iss` checks, a descriptor-safe private session cache, a no-follow cross-process refresh lock, noninteractive denial, logout, and a verified no-clobber chunked download. |
| `airs-cli-link-probe` | The real `airs` library linked with the client skeleton, for the size delta and the CLI+client feature graph. |

## 2. Toolchain and pins

Rust `1.89.0` (MSRV) for fmt, Clippy (`-D warnings`) and tests. Resolved:
rmcp 3.8.0, jsonwebtoken 11.1.0 (aws-lc-rs 1.19.0), axum 0.8.9, jiff 0.2.39 with
jiff-tzdb 0.1.9 (embedded tzdb), tokio-rustls 0.26.6, oauth2 5.0.0, and reqwest
0.13.5 (single version). The lockfile resolves 300 packages.

## 3. Results (round 7, after review 6 corrections)

Receipts for this round: `.local/dev/runtime-mcp/m0/r7/`. Earlier rounds' receipts
are kept unchanged in `m0/r6/` … `m0/r2/` and `m0/` (round 1). Exact commands, run from
`prototypes/runtime-mcp/` unless noted:

```sh
cargo +1.89.0 fmt --all --check
cargo +1.89.0 clippy --locked --all-targets -- -D warnings
cargo +1.89.0 test --locked -- --nocapture
cargo package -p airs-runtime-core -p airs-mcp-client --allow-dirty   # cargo 1.99.1, local overlay for the unpublished core
# repository root; prototype crates added as root workspace members in a copy (target/m0-unified/src):
cargo clippy --workspace --all-targets -- -D warnings
AIRS_REQUIRE_PARITY=1 cargo test --workspace --no-fail-fast
```

| Check | Result |
| --- | --- |
| Prototype fmt / Clippy (Rust 1.89.0, and Clippy also on 1.99.1) | pass / pass / pass |
| Prototype tests (Rust 1.89.0) | **50 passed, 0 failed** |
| Package verification outside the workspace | pass (`package-check.log`, `package-hashes.txt`). Core's packaged manifest depends on registry `scm-sdk =0.2.4` and ships `schemas/*.json`. Publication order: core → client → CLI. |
| crates.io names | all four unclaimed; HTTP 404 per name (`crate-names.jsonl`) |
| Unified root workspace (copy + prototype members, full TS reference) | see `r7/unified-*.log`; summarized in the phase ledger |
| `serde_json/preserve_order` | not enabled in any graph |
| Duplicate `reqwest`/`scm-sdk`/`serde_json`/`tokio`/`rustls` | none |
| SQLite in the client/core graphs | none (the CLI still has it until M1) |
| Release size (remeasured in round 3, `r3/binary-size.json`) | 45,773,672 → 48,180,328 bytes (**+2,406,656 B, +5.26%**). Identical to round 1: the client skeleton (including its 256 KiB result check, present since round 1) is unchanged. This measures the *skeleton* client kept reachable by a probe; M7 remeasures the real client. |

The prototype tests comprise:

- **core:** 13 lib tests (schedule/DST, envelope, and 6 executable authority-model
  flows) and 8 schema-fixture tests (catalogue/authority classes, closure, examples,
  envelope mutations, closed job-result variants, request-decoder agreement,
  error-envelope agreement, constrained identifiers in results);
- **server:** 9 lib tests (JWKS spacing/outage/expiry, URL refusal, storage
  admission, owner lock, row-verified backup drill, reservation release on drop,
  deployment-template validity with loopback-only hooks refused) and 9 loopback
  suites;
- **CLI:** 5 unit tests (URL trust, session-file and lock safety, resource binding,
  the shared complete-result bound) and 6 end-to-end suites.

Feature-set differences from the CLI-only graph:

- CLI + client: reqwest gains `json`.
- Unified with server and tests: reqwest `form`; serde_json `alloc`; rustls
  `log`/`logging`; tokio `process`.
- **Lock difference:** the isolated prototype lockfile resolved tokio **1.54.1**,
  where the root lock has **1.53.3**. In the unified copy the root lock governs, so
  tokio stays 1.53.3 unless a later change updates the root lock deliberately.

Loopback measurements of the **complete** serialized `CallToolResult`:

- Agent pages: 5 pages; largest complete result **15,672 B**.
- CLI chunk: a 64 KiB raw chunk has a complete result of **175,406 B**.
- Echo: 120 KB of text fits the bound; 200 KB is refused locally, as the runbook expects.
- 32 MiB artifact: 512 calls, SHA-256 verified; about **11.4 s** direct and **10.9 s**
  through the CLI → fixture gateway → spike path, on a debug build over loopback.

Round 1 had reported 87,654 B. That figure measured only the structured value and
missed rmcp's duplicated text content; review 1 found the error.

## 4. What the negative tests prove

**Server identity:** 28 tabled cases plus `jku` and HS256 are denied before MCP
dispatch, and a good pair is accepted. The cases cover:

- headers: missing/duplicate credential headers; a non-Bearer scheme;
- upstream audience, tenant, gateway client and owner, including an allowlisted
  owner that the grant does not name;
- app-only tokens: roles-only, and `idtyp=app` even with `scp`; a missing permission;
- expired, or missing `nbf`;
- the assertion: for another backend; without `aud`; with a wrong issuer; without
  `iat`; lifetime 301 s over the 300 s ceiling, or far longer;
- an unknown agent, an unknown caller, or a missing agent ID; a wrong workspace or
  organisation;
- a rogue key reusing a known `kid`; the gateway key used as upstream.

**JWKS:** an initial outage causes one fetch for ten requests. An expired cache keeps
serving the stale set while the source is down, with one fetch per spacing window.
Unknown `kid`s cannot storm the source.

**Authorization:**

- Token scope tier is intersected with grant tier, both ways, with positive controls.
- An operator without the tool in its allow-list is denied.
- Admin mutation is disabled by default.
- An administrative identity cannot reach the lower-tier `operator_noop` in either
  mode, even holding a valid proof.
- Under `fresh_proof`, these are denied: a cached session without proof, stale
  `auth_time`, changed parameters, replay, and a *different fully authorized
  administrator* using an unconsumed preview's nonce. Exactly one bound execution
  succeeds.

**Sessions:** inside one live MCP session (session ID reused), the full matrix is
refused: {expired assertion → 401, unknown agent → 403, no credentials → 401} ×
{POST, GET, DELETE}. The good credentials still work afterwards.

**Session ownership:** each MCP session is bound to the principal that created it.
A *different valid, granted* principal (positive control: it can open its own
session) gets 403 on POST, GET and DELETE of another's session, and the owner's
session keeps working. A refreshed token for the same identity is the same owner.
After the owner's DELETE, or for a guessed ID, the server's own guard answers 404
`unknown session` before the transport is reached.

**Session capacity** (bound plus in-flight creations) is reserved atomically. With
one free slot and 32 creating requests held open concurrently (a loopback-only
delay hook, refused by configuration validation off loopback), exactly 1 succeeds
and 31 get 503. A DELETE releases capacity. Failed creations leak no reservation,
and a dropped (cancelled) request releases its slot.

**Envelope and authority model:**

- The compiled decoder round-trips every example request losslessly. It refuses
  unknown fields, other versions, invalid tenants, a missing or malformed
  key/epoch on idempotent tools, and a key/epoch on non-idempotent tools (including
  an explicit `null`). Another epoch returns `control_restored`.
- The compiled `ErrorEnvelope` decodes every schema-valid error (all 17 codes, with
  `credential_status`) losslessly. Its code set equals the schema's. Both the schema
  and the decoder refuse: an extra field, an unknown status, a malformed or
  uppercase `job_id`, a non-UUIDv4 suffix (36 hyphens, wrong version or variant,
  misplaced hyphens), an explicit `null` optional member, and a negative retry delay.
- The executable model (`src/authority.rs`) accepts the direct and approval flows
  and the complete upload flow (begin → chunks → finalize → separately proven
  import). Exactly one import is admitted per upload: ownership, expiry and the
  finalized/unconsumed state are rechecked inside the consuming transition, so two
  overlapping previews admit one job, and an abort or expiry after a preview blocks
  it. Exact idempotent retries return the original
  job or upload ID; retries with a changed digest, generation, preview or approval
  are `IdempotencyConflict`. It denies:
  - cross-operation proof reuse, foreign actors, stale or missing proof;
  - digest or generation changes, and expiry;
  - approval reuse, approval transfer and self-approval;
  - `upload_admit` previews on the lifecycle execute and approval paths, and
    non-upload previews on upload begin;
  - `import_finalize` previews without a finalized upload, or from another actor;
  - foreign chunks, finalizes and aborts; replayed offsets; oversize, incomplete or
    hash-mismatched uploads; abort after import.

**CLI:**

- URL trust: HTTPS only, or loopback HTTP; the resource origin and every discovered
  authorization/token/issuer/registration endpoint must be on an approved origin.
- Discovery itself is guarded at the request layer: an `OriginGuard` around rmcp's
  OAuth HTTP client refuses every request to an unapproved origin before sending,
  and never follows redirects. Protected-resource metadata naming a second loopback
  origin as the authorization server leads to **zero** requests to it; a control
  request proves that listener is reachable.
- Endpoints on an unapproved origin are refused before any authorize request, and
  downgrades are refused before any network access.
- The session cache refuses symlinks, hard links, oversize files, loose modes and
  unsafe ancestors, and reads through the verified descriptor. The refresh lock
  refuses symlinks and loose files.
- A session for another resource is never reused.
- Wrong `state` (CSRF) and wrong `iss` (mix-up) callbacks save no session.
- **Two concurrent `refresh` processes serialize:** 2 refreshes, 0 rejected,
  against a fixture that rotates single-use refresh tokens with a 400 ms response
  delay.
- PKCE S256, RFC 8707 `resource`, 0700/0600 cache modes, no token values in output,
  noninteractive denial, logout.
- A verified, no-clobber 32 MiB download through the gateway hop, with a client-side
  256 KiB complete-result bound. A download that fails before any chunk leaves no
  partial file.

**Mutation checks** (`r7/mutation-checks.log`): **34 of 34 caught** by test
failures. Compile errors are never counted as catches. Each mutation disables one of:

- backend binding, grant owner, workspace, app-only refusal, session-owner binding,
  the unknown-session guard, atomic session-capacity reservation, reservation
  release;
- the token-tier intersection, the allowed-tool list, the admin lower-tier rule;
- the fresh-proof nonce, the proof replay guard;
- JWKS refetch spacing, drill row-content verification;
- the approved authorization-server origin check, the cross-process refresh guard;
- the session-file hard-link check;
- the discovery origin guard, the shared client result bound, the download
  partial-file cleanup guard;
- the envelope epoch requirement, the epoch match, explicit-null refusal, error
  `job_id` validation, the UUIDv4 version check, the shared schedule-ID pattern;
- the model's proof nonce, approval-agent binding, retry-binding comparison,
  `upload_admit` path restriction, finalize hash check, import recheck at
  execution, and upload expiry.

In round 1, two mutations initially survived (owner constraint, app-only refusal).
Cases were added and both are now caught.

## 5. Findings that bind later phases

1. **rmcp logs the raw authorization code at `debug`** (`exchange_code_for_token_with_issuer`).
   The contract forbids any subscriber that emits rmcp debug events (§1, rule 5).
2. **Gateway backend binding is likely absent** (source inspection; contract §2.2).
   The spike enforces a signed `aud`, so M0.6 will either prove the binding or fail P1.
3. **No step-up/`auth_time` support** in Bonbon or the gateway configuration, so admin
   option B is not expected to qualify; admin stays `disabled` (contract §3).
4. **The Longhorn backup target is unencrypted** (NFS → nas1 → R2). The contract
   adopts an encrypted Longhorn class for the data volume, with Conjur custody plus
   offline escrow, pending M0.6 P8.
5. The baseline root Clippy failed on dead code in the unfinished B4 transfer module.
   M0 scoped `#[allow(dead_code)]` to that module and to `validate_transfer_fact`,
   with no behavior change (§6). The code moves to `airs-history-store` in M1.
6. Bonbon accepts app-only tokens (`MCP_ALLOW_APP_ONLY=true`) and leaves the
   assertion's `iss`/`aud` unchecked. airs-mcp intentionally does neither.

## 6. Root repository changes in M0

- `src/history/mod.rs`, `src/history/repository.rs`: scoped dead-code allowances
  (above). No behavior change.
- Documentation only otherwise: this record, the contract, the runbook, the phase
  review ledger, a DESIGN §22.8 roadmap and a COMPATIBILITY §12 fixture inventory.

## 7. Limits of this evidence

Loopback fixtures model Entra, the gateway and Idira from source inspection; they
are not those systems. No real token, gateway, Claude Code session, Longhorn volume,
backup target or node was exercised. Timing is debug-build loopback timing. The
unified parity run used a copy of the working tree plus the prototype crates, not
the production M1 crates. The size delta measures the skeleton client and will be
remeasured in M7.
