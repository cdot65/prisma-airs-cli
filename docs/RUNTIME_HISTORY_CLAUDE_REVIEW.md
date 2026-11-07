# Claude Code review: runtime history implementation plan

## Review 1 — 2026-11-04

Reviewer: local Claude Code, read-only plan review.

Reviewed plan SHA-256: `35aef1a63079032156ca4ac365c0db807fa7900531df72ae08f0f5733909f374`.

This is the original unsuccessful assessment. Subsequent reviews are appended;
implementation remains gated on a Claude Code score of at least 9/10.

# Independent review: `docs/RUNTIME_HISTORY_IMPLEMENTATION_PLAN.md`

Scope of the review: I read the plan, `AGENTS.md`, `RUNTIME_HISTORY_PLAN_REVIEW.md`, DESIGN §3.3–3.9 and §18.11–18.14.2, and the `8.1.0-rc.3` release notes. I also read `src/clients.rs`, `src/tenant/config.rs`, `Cargo.toml` and `Cargo.lock`, `scripts/release.sh` and `scripts/npm-package.sh`, plus the immutable SDK files `scanner.rs`, `dashboard.rs` and `models/generated.rs`. I did not open any validation bundles, tenant files, credentials or environment secrets. I made no network calls and wrote nothing.

## Overall score: 7/10

**Decision: Not accepted for implementation. Revise and rescore.**

The plan is careful and mostly well-structured. The parts that hold up well:
- The 8.1/8.2 split.
- Not trusting names when linking identities.
- Keeping populations separate (aggregate API totals vs locally counted scans).
- Keeping the report budget.
- Treating pruned intervals as gaps, not zero-event windows.

I don't agree with the earlier 9/10, though. Several problems can't be pushed into A0/B0, because they decide what A0/B0 is even asked to produce. Two are compatibility regressions in a minor release, one breaks a stated invariant, and one is a double-counting risk.

---

## Plan-level blockers

These must be fixed in the plan text itself. A0/B0 can't settle them because their scope depends on the answer.

### P1. 8.1 silently drops scan results for every existing tenant with a single flat key
*§Credential routing ("Never use an unbound legacy credential as an automatic fallback for management-discovered applications"); §Evidence and release boundary ("Keep the current live report workflow"); A1.*

Today `runtime report` uses the tenant's flat `airsApiKey`/`airsApiToken` (`src/clients.rs:1168`, `build_scanner_bounded`) to look up results for discovered apps. Scan results are a **required** source (DESIGN §18.14). Under the 8.1 rule, every existing tenant would see all scanner results turn into "explicit gaps" overnight, and `--strict` runs would go from exit 0 to exit 1. That breaks "keep the current live report workflow" in a minor release.

The plan needs to choose and state one migration path. Options:
- an explicit one-time binding command (flat key → app IDs) plus a first-run diagnostic that names the fix;
- an explicit per-invocation opt-in to use the flat credential, which keeps today's behaviour;
- or declare it breaking and move it out of a minor release.

Whichever is chosen, it needs an upgrade-notes requirement and a DESIGN §12-style compatibility row.

### P2. Older binaries can't read the extended TOML at all
*§Credential routing ("TOML continues to be the only configuration source. Extend it…"); A0 ("migration-free reading of existing valid tenant files and canonical new writes").*

DESIGN §3.3 step 2 and `src/tenant/config.rs` (`read_rule`, the strict 27-key schema) treat any unknown key as making the whole file invalid. As soon as 8.1 writes a named-credential table, 8.0.x and the unpublished 8.1-rc binaries fail on every command for that tenant, including management-only commands. That matters because users have Homebrew and npm installs side by side.

The plan covers old file → new binary, but not new file → old binary. It must either:
- state the required property (e.g. "8.0 can still read a tenant's management/flat settings after 8.1 adds named sets") and its consequence (a separate sidecar file such as `tenants/<name>.scanners.toml` under the same safety, lock and transaction rules); or
- explicitly accept that downgrades break, with upgrade notes.

A0 currently has no instruction to decide this.

### P3. A named credential set is more than a key, but the plan doesn't say so
*§Agreed product contract, "Scanner credentials"; A0 grammar bullet.*

Scanner endpoints are regional, and the SDK's `ApiKey` carries `dp_name` (deployment profile) and `expiration` (`generated.rs:4680`). Apps in one TSG can live behind different scanner endpoints. A set defined only as key/token plus app IDs would route correctly by credential and still send requests to the wrong region.

The plan should define a set as: secret (key **or** token) + scanner endpoint + retry setting + associated app IDs. It should also state that the endpoint follows the existing D48 write rule.

### P4. WAL mode contradicts "readers don't write"
*§SQLite ownership ("report/history readers do not … modify"; "selecting a read-only open flag alone is not proof that SQLite creates no files"); B1 test "no files created by readers".*

In WAL mode every reader writes to `-shm` (read-mark slots), and it creates `-shm` if it's missing and the directory allows it. That is how WAL works, not an edge case to qualify away. So the plan's phrasing makes a promise WAL physically can't keep, and the B1 test would pass only by luck of fixture setup.

The plan needs to state the invariant precisely, choosing one of:
- (a) readers may update an existing `-shm` but never create, chmod or delete files, and refuse with an actionable message if sidecars are missing or unsafe; or
- (b) use rollback-journal mode, where readers take only shared locks, accepting writer/reader blocking.

B0 can then pick the mechanism. The plan must state which invariant is being prototyped.

### P5. Scans retained from the CLI will be double-counted against later synced events
*§Records ("Do not merge … same scan IDs across unsupported identity relationships"; "Missing identities are quarantined"); B2; Product contract "count scans independently of observations".*

The pinned `ScanResponse` (`generated.rs:166553`) has `scan_id`, `report_id` and an optional `session_id`/`tr_id`, but **no app ID or exact app name**. `ScanIdResult` has no app identity either. So every B2-retained scan lacks the five-part identity and ends up quarantined. A later `runtime sync` discovers the same scan with full dashboard identity. With no merge rule, the plan's own rules produce two scans.

The plan must require a deterministic **promotion/reconciliation** rule: when a quarantined partial-identity record and a fully identified one share a cloud namespace and `scan_id`, and nothing contradicts the link, they become one scan. Contradictory cases stay as attributable conflicts. Population counts must never include both. B0 should spell out the exact predicate. The existence of the rule, and the ban on double counting, belong in the plan.

### P6. The sync and backfill model doesn't match what the pinned SDK can actually do
*§Sync ("Backfill accepts only windows representable by actual pinned SDK requests"; "Persist stable work descriptions"); B3.*

The SDK's dashboard and session queries use rolling `time_interval`/`time_unit` with `limit`/`offset` (`dashboard.rs:34–68`). There's no absolute start/end and no cursor. That has consequences the plan should state rather than leave to B3 tests:
- **Backfill can't resume across invocations.** Offsets shift as new sessions arrive. An older window can only be reached through a larger rolling window that includes all newer data. A truncated backfill re-reads the head of the list next time.
- **The current walk aborts on change.** It fails with "Unstable pagination" when `total_items` changes mid-walk (DESIGN §18.13). On a busy tenant an hourly sync will hit that routinely, so the plan needs a tolerant-traversal policy: dedupe, mark the segment incomplete, never fail the whole run.
- **Window edges are approximate.** Boundaries come from the server's "now"; the client only knows its own clock. Coverage segments therefore carry boundary uncertainty, and the overlap must cover skew plus ingestion latency.
- **Sync can fall behind.** If a tenant generates more sessions or scans per interval than the request and page budget can cover, sync never catches up. That needs a visible "falling behind" coverage state, not a quietly growing gap.
- **"Traversed" isn't "settled".** A segment traversed at time T is not complete forever. Late events past the overlap are lost unless the segment status records "traversed as of T".

B0/B3 can set the numbers. The plan must state these limits so B3 isn't scored against a resumable-backfill promise the SDK can't keep.

### P7. Nothing stops two syncs running at once
*§Sync; B6 ("overlap prevention" only in the scheduler examples).*

Because network waits must happen outside write transactions, SQLite's locks can't stop a scheduled sync and a manual `--refresh` from both spending API budget and racing on progress. The plan needs a process-level, per-archive sync lease (e.g. `flock` on a private lock file under the data root, same safety rules as `config.lock`). It should also define what happens on contention (refuse vs wait), separately from SQLite busy timeouts. The persisted 429 cooldown needs the same per-archive scope.

### P8. Credential selection isn't defined for scan, bulk-scan and resume-poll
*§Credential routing; A0/A1; B2.*

The plan defines selection for direct lookups but not for submissions. Specifically:
- A tenant with only named sets needs a selector on `scan` and `bulk-scan`.
- `resume-poll` must poll with the credential used at submission. Today's bulk state files (DESIGN §14.9) don't record a credential reference, so adding one changes the state format and needs a compatibility rule for existing state files.
- The B2 app association for retained scans depends on this selection.

A0 is the right place for the grammar, but the plan must add these commands to A0's scope.

### P9. Rotation can strand old scans in the enrichment queue forever
*§Credential routing ("rotating a credential affects future requests"); Product contract "Key expiry/rotation does not erase history".*

If the qualification shows a rotated key can't read results for scans submitted under the old key, then under "never try keys in sequence" those scans can never be enriched. Bounded retry is only specified for report examples, so result enrichment would retry them on every run.

The plan must:
- define a terminal state such as "credential-ineligible" for result enrichment;
- decide whether a set may hold more than one validity-scoped secret; if it may, that needs reconciling with "never try keys in sequence", e.g. by selecting the credential deterministically from the scan's submission time or reference.

---

## Contract work correctly assigned to A0/B0

These are acceptable as deferrals. The rescored plan should keep them gated and add the bracketed items to the phase bullets. None of this review implements them.

**A0:**
- A field-level identity table covering `ApiKey.customer_app_id`/`cust_app`, `CustomerApp.customer_app_id` and the dashboard app ID/name buckets, with proven vs unverified links.
- The exact TOML grammar for named sets, built on the P2 decision.
- Masked `tenant read`/`get`/doctor views, keeping the `{key, value}` JSON shape stable for scripts or recording a §12 row.
- Behaviour when a parse error involves a secret.
- **[add]** The credential reference stored in history must be the local set name, plus `api_key_id` only if proven. **Never `api_key_last8` or `auth_code`**: `last8` is a fragment of the secret and must not be used to match keys either.
- **[add]** The submission-command selectors and state-file compatibility from P8.

**B0:**
- Data root path and precedence; the canonical cloud identity derived from the management endpoint; how aliases and custom environments map.
- Schema and indexes; migration and backup format; export format (logical, non-SQL; see the earlier review).
- The journal mode chosen under the P4 invariant.
- Busy timeouts and checkpoint ownership.
- The tolerant-traversal algorithm and overlap length (P6); cooldown scope and clock handling.
- Exit codes for missing, empty and uncovered archives.
- **[add]** What happens to today's live `report` flags (`--interval`, `--unit`, `--max-pages`, `--max-reports`, `--max-events`, `--app-id`) once local history is the default. Recommended: they're a usage error unless `--refresh` is given; never silently ignored.
- **[add]** Whether `--refresh` performs example enrichment.

**B6:** numeric benchmark thresholds (already required before acceptance; fine).

---

## Required changes to reach ≥9

1. **P1:** add a stated migration path for flat-credential tenants, with upgrade notes, a compatibility row and a strict-exit regression test.
2. **P2:** state the old-binary compatibility requirement and the design consequence, or explicitly accept breaking downgrades.
3. **P3:** define a credential set as secret + scanner endpoint + retries + app IDs.
4. **P4:** state the reader-side filesystem invariant precisely (`-shm` updates vs file creation) and make B1's test assert exactly that.
5. **P5:** add the partial-identity promotion rule and a "no double counting" invariant; add B2/B3 tests for a CLI-retained scan that sync later discovers.
6. **P6:** add realistic pinned-SDK sync limits: non-resumable backfill, tolerant traversal of shifting pagination, uncertain window edges, a falling-behind state, traversed-vs-settled segments.
7. **P7:** add a process-level per-archive sync lease and contention behaviour.
8. **P8/P9:** add submission/resume-poll credential selection and state-file compatibility to A0; add a terminal state for result enrichment after rotation.
9. **Platform gates:** the release ships **three** targets (`scripts/release.sh:32–33`): `x86_64-unknown-linux-musl`, `aarch64-unknown-linux-musl` and `aarch64-apple-darwin`, the Linux ones cross-built with `cargo zigbuild`. B0's prototype and A3/B6 qualification only name Linux amd64 musl and macOS arm64. Add aarch64-musl: cross-compiling bundled SQLite C under zig cc, plus an execution check on a native host or emulator. Also benchmark on the musl build: its allocator behaves very differently from glibc at 1M rows.
10. **Prune privacy:** "requested-only deletion" must include removing deleted content from free pages (`secure_delete` and/or an explicit `VACUUM`, with its 2× disk and exclusive-lock cost documented). Otherwise pruned snippets stay recoverable in the file and in exports of raw pages.

---

## Lesser recommendations

- **B2 opt-out and failure noise.** Automatic retention runs on every scan in CI, containers and on read-only home directories. Add an explicit opt-out (a flag and/or tenant setting) and rate-limit the "archive unavailable" warning so a `bulk-scan` doesn't emit one per item. An opt-out doesn't contradict the approved "automatic" default.
- **B2 "do not archive submitted payloads" vs "bounded sanitized snippets".** CLI scan results include masked prompt/response data derived from the user's own payload. State whether that is retained as snippets or dropped.
- **Filter dimensions in B5.** A date × app × detector × verdict × action summary can blow past the 32 MiB HTML cap. State a cardinality budget and the fallback order: restrict to the detail view, then top-K with disclosure.
- **CSP change.** Moving from "no scripts" (DESIGN §18.14.1) to hashed inline scripts needs a §12/DESIGN note. Filters should be progressive enhancement, so the no-JS and print views stay complete.
- **Filesystem placement.** Refuse, or at least warn, when the data root is on NFS/SMB or a cloud-synced folder; WAL and locking are unreliable there.
- **Result batching.** With per-app clients, five-ID batches fill poorly when apps have few candidates. Make A2 fairness account for batch efficiency against the shared `--max-requests`.
- **Credential expiry.** `ApiKey.expiration` and `rotation_time_*` exist in the SDK. A0 could optionally surface an upcoming-expiry diagnostic using that metadata only.
- **Review gate wording.** "Independent Codex phase scores" should say "independent reviewer", so reviews from other tools (like this one) are recorded consistently in `RUNTIME_HISTORY_PLAN_REVIEW.md`.

---

## Why 7 and not 9

The previous review rated the plan "sound, provided A0/B0 finish". In my view P1, P2, P4 and P5 aren't open questions for A0/B0 to answer:
- P1 and P2 are compatibility commitments the plan currently breaks without saying so.
- P4 is an invariant the plan states in a form SQLite WAL can't satisfy.
- P5 is a counting error that follows from the plan's own rules and the pinned SDK's `ScanResponse` shape.

P6 needs the plan to describe sync realistically before B3 can be judged fairly. Once these are fixed in the text, the rest of the plan's structure would support a score of 9.

## Review 2 — 2026-11-04

Reviewer: local Claude Code, read-only reassessment.

Reviewed plan SHA-256: `5c3cb5a143d7cbe3b60f5cf1a348d32ce0a044d23e45becd2c4254a88ce2dd01`.

# Claude Code re-review: `docs/RUNTIME_HISTORY_IMPLEMENTATION_PLAN.md` (Revision 2)

This was a read-only plan review. I read the revised plan, my original review, `src/clients.rs` (scanner construction), `src/bulk/state.rs`, `src/store/` (tenant index), `Cargo.toml`, and the immutable scm-sdk 0.2.4 files `configuration.rs` and `auth.rs`. I wrote no files and ran no commands. I did not open tenant config, credentials, `signal-brief-*` bundles or any live API.

## Overall score: 8/10

**Decision: Revise before implementation.** The revision fixes every original plan-level blocker (P1–P9) in substance, and the corrections fit together. That is a real improvement over 7/10. It still falls short of 9 for three reasons:

- The new P1 remedy (`bind-default`) contradicts how the pinned SDK actually sends credentials.
- The new prune-privacy text contradicts itself on where SQLite temporary data goes during VACUUM, and that matters at the 1M-scan benchmark scale.
- Choosing the DELETE journal mode brings a reader/writer starvation problem, including for automatic retention, that the plan doesn't bound.

Each is a short text fix. None is structural.

## Original blockers

| # | Status | Assessment |
|---|---|---|
| **P1** Flat-key tenants lose scan results | **Resolved, with new issue N1** | The plan picks an explicit migration path (lines 96–107): `bind-default` writes locally with no network request, unbound reports name the remedy, and it requires a §12 row, upgrade notes and paired strict-exit tests. Saying "do not promise identical strict results before binding" is honest. Two leftovers: line 35 still says "Keep the current live report workflow" without that qualifier, and the dual-secret case is undefined (N1). |
| **P2** Old binaries can't read new TOML | **Resolved** | Breaking downgrades are explicitly accepted, opt-in, with a no-clobber private snapshot taken under the config lock, an explicit identity-checked restore through `Transaction`, and no silent restore. This is consistent with the contributor rule against sidecar config. I checked that tenants are enumerated from the index rather than by scanning a directory, so a snapshot file can't be misread as a tenant. |
| **P3** Credential set is more than a key | **Resolved** | Lines 53 and 124–129: one secret, its own endpoint under D48, retries 0–5, explicit app association, no regional inference, and the shared-budget retry override is kept. |
| **P4** WAL vs "readers don't write" | **Resolved** | DELETE journal mode is chosen; readers take shared locks only and never create or update files; a hot journal is refused, never recovered; WAL archives are rejected without being changed. B1's tests now assert exactly this. A read-only connection that finds a hot journal fails with `SQLITE_READONLY_ROLLBACK`, so refusing is feasible. Side effect: N3. |
| **P5** Double counting of CLI-retained scans | **Resolved** | The plan has a partial-observation class excluded from attributed totals, a promotion predicate that needs exactly one supported candidate with no conflicting supplied fields, and atomic removal of provisional eligibility. It correctly treats the named-credential association as request context, not response data. B2 tests the sync-discovers-receipt case. |
| **P6** Sync model vs pinned SDK | **Resolved** | Lines 56 and 265–282 state everything required: backfill can't resume across runs (it restarts at the head of the rolling window), tolerant traversal applies to sync only, structural errors stay fatal, edges are uncertain and need overlap, there is a falling-behind state, and segments are "traversed as of T", not settled. Using "resumable" in lines 37 and 55 is acceptable given the definition at line 56. |
| **P7** Concurrent syncs | **Resolved** | There is a per-archive flock lease taken before any network work (including `--refresh` and examples). Contention fails fast with exit 1, it is separate from SQLite busy timeouts, the cooldown is scoped to cloud/service endpoint across credential renames and rotations, and the cross-host limit is stated. |
| **P8** Selection for scan, bulk-scan and resume-poll | **Resolved** | `--scanner` is added; a tenant with only named sets requires it; resume-poll records the reference, app and endpoint, and refuses on removal, rebinding or endpoint change. A versioned state format is safe, because `bulk/state.rs` uses `deny_unknown_fields` and refuses newer versions, so 8.0 will refuse rather than fall back to the flat key. |
| **P9** Rotation strands old scans | **Resolved** | One active secret per set; references use set name, revision and a proven `api_key_id` only (never `last8`, `auth_code` or hashes); a credential-ineligible/deferred state; re-eligibility only on a changed binding or an explicit reset; post-rotation access is recorded as unqualified. |

| Required change | Status |
|---|---|
| 1–8 | Resolved, as in the table above. |
| 9: three shipped targets | **Resolved.** Lines 182, 450–453 and 470–472 cover aarch64-musl with zig cc and native or emulated execution; B6 benchmarks the amd64-musl build. |
| 10: prune privacy | **Mostly resolved.** `secure_delete`, logical-only export, explicit VACUUM under the lease, and the SSD/snapshot limits are all stated. Remaining problems: N2 and the verification scope (recommendation 3). |

## New blockers

**N1. `bind-default` assumes a single flat secret, but the SDK sends both.**
- In scm-sdk 0.2.4, `Authentication::Scan` puts both `Authorization: Bearer` and the API-key header on the request when both are configured (`auth.rs:748–760`).
- `build_scanner_bounded` passes both from the flat config (`clients.rs:1179–1183`).
- So there is no "flat dual-key/token precedence" (plan line 129): the server decides which one it honours.
- `bind-default` "copies the flat scanner selection" into a set that must hold "exactly one secret type", which is undefined when the flat config holds both.

Required:
- State that `bind-default` refuses when both flat secrets are set unless the operator explicitly picks `--use key|token`.
- Disclose that the result may behave differently from today's dual-header requests.
- Fix the wording at line 129 to "existing SDK behaviour (both headers sent)".
- Add an A1 test for a dual-secret flat tenant.

**N2. `temp_store` in memory contradicts the VACUUM disk budget.**
- Line 385 keeps SQLite temporary data in memory.
- Line 389 budgets "up to twice the archive size in free space" for VACUUM.
- VACUUM builds its copy in a temporary database, which follows `temp_store`. With MEMORY, compacting a multi-GB archive needs about one archive's worth of RAM, not disk.
- Large report sorts and temporary indexes at 1M scans also go to RAM.

Required: pick one approach and say so:
- either compaction uses a private 0600 temp location on the same filesystem inside the 0700 data root (scoped `temp_store=FILE` for maintenance only), and its free-space budget is stated;
- or compaction uses `VACUUM INTO` a private no-clobber file, then an atomic replace under the writer lease, with the rule that readers refuse during the swap.

B6 must also report peak memory for 30/90-day reports and for compaction.

**N3. DELETE mode lets long readers starve writers, including automatic retention.**
- In rollback-journal mode, a writer needs EXCLUSIVE to commit. Any open SHARED lock blocks it, and a writer waiting in PENDING blocks new readers.
- A multi-year or 90-day report over 1M rows is not "short contention". It can stall `bulk-scan` retention and sync.
- Since retention failures only warn, receipts can be lost quietly.

Required:
- State the policy: bounded read transactions (with a target ceiling measured in B6), plus a bounded busy wait for retention.
- Retention outcome when the archive is busy is "skipped and counted in the one per-invocation warning", with no retry spool. Alternatively, define a spool and its safety rules.
- Add a B1/B6 test for retention under a long concurrent report read.

## Exact changes to reach ≥9

1. **N1:** dual-secret refusal or `--use` choice in `bind-default`; correct line 129; add an A1 test.
2. **N2:** remove the `temp_store` vs VACUUM contradiction by choosing the temp location or the `VACUUM INTO` strategy; add the memory budget to B6.
3. **N3:** state the read-transaction bound and the retention-on-busy behaviour; add a contention test.
4. **Line 35:** change "Keep the current live report workflow" to say it is kept except for the unbound-credential strict-coverage change in §Credential routing.

## Lesser recommendations

1. **Upgrade notes:** state that `bind-default` (the P1 remedy) is itself the first named-table write, so it triggers the P2 downgrade limit and the snapshot. The confirmation should say so.
2. **Duplicated secret:** `bind-default` duplicates the secret, so later flat-key rotation won't reach the named set. Document this and have doctor flag flat/named divergence by revision, without comparing secrets.
3. **Prune verification:** check the whole database file, not just free pages, because secure_delete also zeroes cell space inside pages still in use. Assert no leftover `-journal` after commit. Document that the DELETE-mode journal briefly held the original pages before being unlinked. Specify `secure_delete=ON`, not `FAST`.
4. **Downgrade test:** say how "new-file/old-binary refusal" is tested, e.g. a frozen copy of the 8.0 key schema in a test, plus an A3 check against the real 8.0.0 artifact.
5. **Snapshots:** they contain secrets. Give them a lifecycle (list, explicit delete) and exclude them from any export or diagnostic.
6. **Promotion in reverse order:** also test a receipt arriving after sync has already recorded the scan, such as a late resume-poll.
7. **Scanner-only tenants:** B0 identity policy should say how they get a cloud-environment namespace for B2 retention. The index requires `tsg_id`, but the environment is derived from a management endpoint they may not have. Otherwise define "retention skipped with one warning".
8. **Wording:** B1 "private sidecar handling" should be "journal/temp file handling", to avoid confusion with the sidecar-config ban.
9. **Unpromoted receipts:** reports should show their count as a separate, labelled population, so CLI-only users see their scans weren't silently dropped.
10. **Review gates:** keep both, as the plan does. The Codex phase gate (≥9 per phase) and this Claude plan gate are separate, and a plan score does not certify code (line 398).

## Review 3 — 2026-11-04

Reviewer: local Claude Code, read-only reassessment.

Reviewed plan SHA-256: `9ee5be3c1147ad07f3831acb1558b6eca09fdbd010f7720a321c0f11a0452d77`.

# Claude Code re-review: `docs/RUNTIME_HISTORY_IMPLEMENTATION_PLAN.md` (Revision 3)

This was a read-only plan review. I read the full Revision 3 plan and my two earlier reviews in `docs/RUNTIME_HISTORY_CLAUDE_REVIEW.md`. For the SDK and source facts I relied on what I had already checked in Review 2: `auth.rs:748–760` sends both headers, `clients.rs:1179–1183` passes both flat secrets, and `bulk/state.rs` uses `deny_unknown_fields`. I made no edits, ran no commands, and opened no tenant config, credentials, `signal-brief-*` bundles or live APIs. I could not write a plan file or exit plan mode from this session, so the review is only in this message. Nothing was appended to the review record.

## Overall score: 9/10

**Decision: Accepted for implementation**, under the plan's own gating. A0 goes first, and each phase still needs an independent Codex score of at least 9/10.

The two gates stay separate. This score only says the plan is coherent enough to start A0. It does not certify any phase or code (line 460), and it does not replace any Codex phase score.

Why it isn't 10: one real coherence gap is left (C1 below). The plan hands it to B0 in general terms, but it doesn't name the case. It doesn't block A0–A3, because 8.1 has no SQLite at all. It must be written into B0's scope before B0 starts.

## Resolution of N1–N3 and the four required changes

| Item | Status | Evidence and assessment |
|---|---|---|
| **N1** Two flat secrets in `bind-default` | **Resolved** | Lines 110–118: it refuses unless `--use key\|token` is given, and naming a kind that isn't configured fails without writing. The plan says the SDK sends both headers and that server precedence is unknown, so behaviour can change. Line 147 now reads "existing SDK behavior (both headers sent)". Line 498 adds A1 tests for both secrets, each explicit choice, and a missing kind. A0 pins this (line 474). |
| **N2** In-memory temp vs the VACUUM budget | **Resolved** | Lines 425–450: ordinary queries use memory-only temp storage within measured limits, with an explicit "not a promise to compact an archive-sized database in RAM". Compaction is `VACUUM INTO` a pre-created, empty, validated 0600 file in the 0700 data root, which SQLite allows. It holds the writer lease plus an exclusive maintenance gate, verifies the copy, fsyncs the file and directory, then does an identity-checked atomic replace. It budgets 2× disk, measures peak memory, and has its own deadline. B6 measures compaction and report memory (lines 637–639). |
| **N3** DELETE-mode reader/writer starvation | **Resolved** | Lines 224–239: every report/history read transaction has a 2 s monotonic limit that includes lock wait. Rendering happens after the transaction is released. If the limit is hit, the command fails and tells the user to narrow the scope. Retention waits at most 250 ms per batch; a busy batch is skipped, counted and folded into one per-invocation warning, with no spool and no hidden retry. B1 (lines 567–569) and B6 test retention under a long concurrent read. |
| **Change 4** Line 35 qualifier | **Resolved** | Lines 35–37 now carve out the "explicit unbound-credential strict-coverage migration". |

### Review 2's lesser recommendations

| # | Status |
|---|---|
| 1 First write triggers the downgrade limit | Done (lines 134–135). |
| 2 Copied secret drifts after rotation | Done (lines 116–118), with no secret or fragment comparison. |
| 3 Whole-file prune check, `ON` not `FAST`, journal lifecycle | Done (lines 425–431). B4 test wording still lags (see L1). |
| 4 Downgrade test method | Done (lines 139–140): a frozen pre-change artifact in A3, plus an earlier schema fixture. |
| 5 Snapshot lifecycle | Done (lines 136–139). |
| 6 Reverse-order promotion | Done in the records contract (lines 268–271). It is missing from the B2/B3 test lists (see L2). |
| 7 Scanner-only namespace | Assigned to B0 (lines 542–544). One concern remains (see L3). |
| 8 "Sidecar" wording | **Not done**: line 556 still says "private sidecar handling" (see L4). |
| 9 Separate count for unpromoted receipts | Done (line 268). |
| 10 Separate review gates | Done (lines 6–14, 458–460). |

## Remaining items: plan-level vs assigned to A0/B0

### No plan-level blockers remain for A0–A3

The 8.1 track (credential grammar, binding, downgrade, selectors, rotation states) is fully specified at plan level. Its remaining details are explicitly A0's job (lines 464–478).

### C1. Required before B0 starts: the 2 s read limit vs whole-archive history reads

Line 224 puts the 2 s limit on every "report/history read transaction". Line 450 exempts only compaction. Logical `history export` of a multi-year archive with 1M scans can't finish in one 2 s transaction. It is a reader, so it falls under that limit. The chunking-with-a-cutoff fix in line 230 is written for reports. It is only safe if retained rows never change after the cutoff. The plan itself allows in-place changes:
- enrichment fills in unknown fields;
- promotion removes provisional eligibility;
- a conflict invalidates an attribution and recomputes counts (lines 264–271, 279–281).

Retention takes only the shared gate, so it keeps writing during a chunked export. An export done that way could mix populations and still get a valid digest.

**Exact change.** Add to the read-limit paragraph and the B0 bullet at line 545 something like:

> "Whole-archive reads (history export, full statistics and prune preview over unbounded ranges) either (a) run as explicit maintenance under the writer lease and the exclusive maintenance gate, reading in bounded chunks while retention skips and counts, with their own operation deadline like compaction; or (b) read from a verified private `VACUUM INTO` snapshot. B0 picks one. Chunked report views are consistent only if B0 shows that every row change after the cutoff is excluded or versioned."

This is a contract assignment, not a structural redesign. That's why the plan is at 9 and not lower. Still, the plan shouldn't enter B0 without it.

## Lesser recommendations

- **L1. B4 test wording.** Line 613 says "removed content absent from live free pages". Align it with line 428: absent from the whole live database, including cell slack, and no leftover journal.
- **L2. Reverse-order promotion tests.** Add "receipt arriving after an already-indexed scan (late resume-poll)" to the B2 test list (lines 584–585) and B3's tests. The order rule shouldn't exist only in prose.
- **L3. Scanner-only namespace default.** Lines 542–543 derive the namespace from the "configured/default management environment". That's a small inference, and it sits awkwardly with line 144 ("no environment/regional inference"). Allow the default only when the tenant's scanner endpoint is also a known default-environment endpoint. Otherwise, treat retention as ineligible and warn once, so receipts can't land in the wrong cloud's archive.
- **L4. Line 556.** Change "private sidecar handling" to "journal/temporary file handling".
- **L5. Writer hold time.** The 2 s reader limit includes lock wait, so long write transactions eat into it. Examples are a large sync batch, or a secure-delete prune of a wide range in one transaction. B0 should set a target write-hold ceiling for sync and import batches. It should also decide whether large prunes are chunked or run under the exclusive maintenance gate, with reader refusal disclosed.
- **L6. Compaction temp storage.** Name the compaction connection's `temp_store`: index rebuilds inside `VACUUM INTO` sort through it. Don't promise temp files inside the data root. SQLite's temp directory is process-global, and changing it safely runs into the no-unsafe-code rule and the hazards of changing environment variables at runtime. Either keep MEMORY with a measured budget, or document the system temp directory (0600, unlinked).
- **L7. Lock order.** Every acquisition is fail-fast or bounded, so deadlock isn't possible today. Still, state one order — writer lease, then maintenance gate, then SQLite — and say the TOML config lock is never held while taking archive locks. That keeps future blocking changes safe. Also define what a reader does when the gate file is missing: refuse, don't create it.
- **L8. 90-day feasibility.** Make "a 30/90-day report at 1M scans completes within the 2 s read limit on documented hardware" an explicit B6 pass criterion. Otherwise "fail and narrow the scope" could quietly fall short of the approved 30/90-day product requirement (line 71).
- **L9. `bind-default --app-id` cardinality.** Line 100 is singular, while the product contract (line 53) says "application IDs". A0 should state whether it can be repeated.

## Gate summary

- **Claude plan gate:** 9/10, accepted. Add C1 before B0 begins. L1–L9 can be folded in during A0/B0 contract work.
- **Codex phase gates:** unchanged and separate. Each of A0–A3 and B0–B6 still needs its own independent score of at least 9/10 with executable evidence. This review implements none of them.
