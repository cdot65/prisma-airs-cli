# Milestones

The goal is full parity with the TypeScript `airs-cli` without the TypeScript CLI
installed. The reference build pinned at tag `v7.3.0` (`7a54e46`, the final TypeScript
release; `5711012`, 7.2.1-forgejo.0, until 8.0) is a frozen specification. A port may only differ from it to fix safety or correctness, and each
difference gets a DESIGN §12 row and a pinning test. At 1.0.0 the executable is
invoked as `airs` (the product, crate and repo keep the name airs-cli), and `airs-cli-rs`
stays through 1.x as a compatibility name.

## Scope

| Area | Decision |
|---|---|
| API-backed commands | All ported natively |
| `--debug` API log | Ported. Secret-redacted, written to `$XDG_STATE_HOME/airs-cli/debug/`; the newest 20 logs are kept |
| `report` | Ported for runtime, redteam, gateway and supplychain |
| Backups and restores | Ported (profiles, DLP, redteam targets). No compatibility with TypeScript-made files |
| `model-security install` | Ported, uv only. `--fetch-uv` downloads a pinned, checksum-verified uv into `~/.local/share/airs-cli/tools/` |
| `model-security pypi-auth` | Ported. The credential-bearing index URL is never printed |
| `completion` | Dropped |
| `redteam judge` and TypeSafe | Dropped (it was a proof of concept) |
| `runtime dlp generate` | Dropped |
| `markdown`, `csv`, `yaml` output | Dropped. Only `json`, `pretty` and `table` exist; anything else is a usage error |
| Node.js and TypeSafe doctor checks | Dropped. A "Gateway API" check arrives with the gateway milestone |

**Renames.** These are breaking changes, with no aliases. They cover commands, help,
doctor checks, report titles and config keys.

| TypeScript | Rust |
|---|---|
| `aigateway …` | `gateway …` |
| `agentguard …` | `supplychain …` |

## Tenants and state (from M4t)

- **Tenant files.** `$XDG_CONFIG_HOME/airs-cli/config.toml` holds the active tenant
  and global settings, and each tenant gets its own `tenants/<name>.toml`. The default
  location is `~/.config/airs-cli/`; `AIRS_CLI_CONFIG_DIR` overrides it.
  - Files are mode 0600 and directories 0700.
  - Writes take a `flock` lock, with pid and host recorded so stale locks can be
    detected, and finish with an atomic rename.
  - Secrets stay in the tenant file for now.
  - Credentials never come from the environment.
- **Rules carried over.** The TSG pin is kept. A `defaultOutput` that names a dropped
  format makes the config invalid.
- **Import.** `tenant import [--from <registry>] [--from-config <file> --name <name>] [--name <name>] [--replace] [--dry-run]`
  reads the TypeScript registry and configs read-only. It renames keys (gateway,
  supplychain), drops and reports a dead `defaultOutput`, `dataDir` and the TypeSafe
  keys, reports unknown keys, keeps the TSG pin, imports the selection only when none
  exists, and never overwrites a tenant unless `--replace` is given. From M4t the CLI
  no longer reads the TypeScript registry directly, and `tenant create --config` is
  replaced by `--from-config`. The binding contract is DESIGN §3 (rows D70–D90).
- **Bulk-scan state.** Kept under `$XDG_STATE_HOME/airs-cli/bulk-scans/`. The
  `dataDir` key is dropped.
- **Reports and backups.** Written to the current directory or `--out`; an existing
  file is never overwritten.

## Roadmap

| Version | Milestone | Status | Scope |
|---|---|---|---|
| 0.1 | M1–M3 | done | Tenant loading and diagnostics; read-only profiles and topics; `runtime scan` |
| 0.2 | M3.5 Foundations | done | `pretty`/`table` renderer for every existing command; `--debug`; Linux musl targets (x86_64, aarch64); Forgejo CI (Linux; macOS on `jadzia` when it is online); signed and notarised release pipeline. The doctor no-retry change (D49) shipped first |
| 0.3 | M4t Tenant writes | done | TOML layout, `tenant create/switch/set/unset/delete`, `tenant import` |
| 0.4 | M4 Runtime writes | done | Topic and profile create/update/delete, `topics apply/eval/revert/sample`, `profiles cleanup` |
| 0.5 | M5 Bulk scanning | done | `bulk-scan`, `resume-poll`: async submissions, checkpoints, locking, recovery of ambiguous submissions |
| 0.6 | M6a Red Team | done | All `redteam` commands except `judge`; `redteam report`; targets backup/restore |
| 0.7 | M6b DLP | done | DLP CRUD; backup/restore |
| 0.8 | M6c Model Security | done | All `model-security` commands, including `install` and `pypi-auth` |
| 0.9 | M6d Runtime extras | done | api-keys, customer-apps, deployment-profiles, sessions, dashboard, scan-logs; `runtime report`; profiles backup/restore |
| 0.9.1 | scm-sdk 0.2.2 adoption | done | Exact pin `=0.2.2` with the same features (only the `scm-sdk` lock entry changes). G25 closed: red team responses no longer enforce string formats and use the frozen TypeScript schemas. The CLI checks every value the 0.93.0 contract refuses before the tenant: `targets error-logs --offset`/`--search` refused, data-plane `--limit` at most 100, the `list` filter and `report --severity` vocabularies, `scan --depth`/`--breadth` at most 20 and `--name` code points (D300–D305, DESIGN §15.23). Response formats are relaxed in every answer, so adapter answers TypeScript refuses on a format are read (D306), and `adapter update` names a stored channel that is not a UUID (D307). |
| 0.10 | M6e Gateway | done | `gateway` (renamed); `gateway report`; doctor "Gateway API" check |
| 0.11 | M6f Supplychain | done | `supplychain` (renamed); `supplychain report`; G71 tolerant scan read |
| 1.0 | Rename and distribution | done | Executable invoked as `airs` (product, crate and repo stay airs-cli), with an `airs-cli-rs` compatibility name; Homebrew tap. (The crate has been on crates.io as `airs-cli` since 0.3.1.) The contract is DESIGN §21 (D326; D73 extended): chunk 1 the rename across code, help, messages, tests and docs; chunk 2 distribution (release tarballs `airs-cli-<version>-<target>.tar.gz` with the compatibility link, the Homebrew formula built from the crates.io crate and its script, RELEASING, README install); scm-sdk `=0.2.3` with the same features (the red team management 0.86.0 update: D327–D329, D35 extended; DESIGN §15.24) |
| 8.0 | The move and the TypeScript 7.3.0 port | in progress | Part A: the Rust CLI moves into `cdot/prisma-airs-cli` (the `airs-cli` name, npm, the docs site; DESIGN §21.15–§21.17). Part B (this port, DESIGN §13.17, §15.25): the reference moves to tag `v7.3.0` (TESTING §3.4); scm-sdk 0.2.4 (the TypeScript 0.35.0 overlay, `=0.2.4` from crates.io); directional security profiles on `profiles create\|update` and `topics apply\|revert`, the 7.3.0 merge, output and revert rules, the views, the runtime report and profile transfer; D332–D336 added, D117 retired, D26, D50, D93, D94, D96, D98–D103, D106, D115 and D116 reworded; the 0.2.4 gap closure (every profile read without response defaults, the restore's 0.35.0 re-check, the multi-page and cumulative 0.2.3 regressions; no §12 change), independently plan-reviewed and scored (below). Done offline (fmt, clippy, the full parity suite, the 1.89 check, `cargo audit`, `cargo deny check advisories sources`); the bounded live run per direction (B7, decision 10) passed against `redtail-prod` on 2026-10-22 after D335 (below); the adversarial review's findings fixed offline, with D336 (below; awaiting its re-check); left: Part C (the 8.0.0 release on every channel) |

## Definition of done for a milestone

1. Every command it adds has executable tests and parity tests against the frozen
   reference. Every difference has a §12 row, a `dNN_` test and a
   [COMPATIBILITY.md](COMPATIBILITY.md) row.
2. `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`
   and `AIRS_REQUIRE_PARITY=1 cargo test --workspace` pass.
3. A bounded live run passes against `redtail-prod` (rules below).
4. An adversarial review has run, and its confirmed findings are fixed.
5. The work is committed and pushed to `main`, then tagged `v0.N.0`.
6. A Forgejo release is published with signed and notarised macOS tarballs, Linux
   musl tarballs, checksums, and notes listing the new §12 rows.

From 1.0 (DESIGN §21), each stable release is also done when:

7. The tarballs are `airs-cli-<version>-<target>.tar.gz`, each holding `airs` and the
   `airs-cli-rs` link, checked by `scripts/package-release.sh` (RELEASING §3, §4).
8. The crate is published to crates.io (`cargo publish -p airs-cli`) after the Forgejo
   release.
9. The Homebrew formula for the version, rendered by `scripts/homebrew-formula.sh` from
   the published crate, passes `brew install --build-from-source`, `brew test` and
   `brew audit --strict`, and is committed to `Formula/airs-cli.rb` in the
   public GitHub `cdot65/homebrew-prisma-airs-cli` repository (RELEASING §6). Pre-releases
   get no formula.

## Live test rules

- **Scans.** Live scans use the profile "Prisma AIRS Harness", at most 8 scans per
  run.
- **Own resources only.** Live write tests change only resources the run created,
  named `airs-rs-e2e-<run id>-…`. They never update or delete anything that existed
  before. Testing an update means creating a throwaway resource and updating that.
- **Never live:** `api-keys create/regenerate/delete`, `redteam eula accept`, gateway
  organisation/admin/scope changes (from 0.10 every gateway write, on either CLI, refused by
  the live session guard), gateway inference (billed model calls), `model-security install`,
  and anything that is tenant-wide.
- **Billed or long-running work** (red team scans, model-security scans, bulk scans)
  needs its own opt-in variable and a per-run budget.
- **Cleanup.** A run creates at most 10 resources. Every create and delete is written
  to `target/live/ledger.jsonl` (IDs and outcomes only). A cleanup command that is
  safe to re-run deletes only prefixed resources older than 1 hour.

## Dependencies and releases

- **SDK gaps.** Gaps or bugs in `scm-sdk` are fixed in `scm-sdk-rs` on a branch, with
  tests. The maintainer reviews and publishes a 0.1.x release, and the CLI then moves
  its exact pin. A git dependency is allowed only while a milestone is unreleased.
  Every tagged CLI release depends only on published crates.
- **macOS signing.** Builds are signed with the Developer ID and notarised with the
  `airs-cli-notary` keychain profile. Signing runs on the maintainer's Mac for now
  and moves to the `jadzia` runner later.

## Notes for later milestones

- **M4t follow-ups.** Done with the store step: AGENTS.md's tenant invariant now
  describes the TOML store, and the CI smoke test uses an empty `AIRS_CLI_CONFIG_DIR`.
  Done with the write step: `tenant create`, `set`, `unset`, `switch` and `delete` on
  the locked atomic store, with prompts, the pty and lock-holder test helpers, and the
  D74–D88 pins. Done with the import step: `tenant import` (registry, `--from-config`,
  `--name`, `--replace`, `--dry-run`), the live tier's private store filled by
  `tenant import` and removed afterwards, `provision_matches_import`, the runner's
  import-source guard, and the import parts of D27, D44, D51, D52, D82, D84 and D85
  (COMPATIBILITY.md §4). Still open for the release: the bounded live run against
  `redtail-prod` (it now exercises the import) and the adversarial review. At 1.0 the
  hint program name becomes `airs` and D73 records it.

- **M4.** The TypeScript `topics create` builds its management client without the
  tenant's endpoint overrides or credential assertion. The SDK's `PANW_MGMT_ENDPOINT`
  fallback applies there, so a loopback tenant is bypassed
  (`docs/research/ts-cli-shell.md` §17). Parity fixtures must account for this, and
  the Rust port must not reproduce it.
- **M4.** Commander's root parser consumes `--output` everywhere, so TypeScript
  `topics create --output json` prints pretty output (`docs/research/ts-cli-shell.md`
  §3). The Rust port honours `--output` and records the difference in §12.
- **M4 (0.4, done).** All eleven commands shipped with their executable, parity and
  `dNN_` tests (DESIGN §13, D93–D117; COMPATIBILITY.md §3.11–§3.14). The live write
  flow, eval flow and sweeper ran against `redtail-prod` with no mismatches and nothing
  left behind. Observed there: every profile PUT creates a new `profile_id`, and older
  revisions keep referencing a topic, so `topics revert` usually keeps the topic: from
  0.4.1 that is exit 0 with a warning (D99) until `profiles cleanup` removes them.
- **M5.** `scm-sdk` never replays async submissions, and it reports post-dispatch
  failures as `RemoteOutcome::Unknown`. That maps directly onto the TypeScript
  "ambiguous" submission state. Port the behaviours of
  `tests/unit/cli/bulk-scan-state.spec.ts`, `bulk-scan-lock.spec.ts` and
  `tests/integration/cli/runtime-bulk-scan.spec.ts` as executable acceptance tests
  first. The state format and lock are Rust-only (no TypeScript checkpoint import).
- **M5 (0.5, released).** `runtime bulk-scan` and `runtime resume-poll` are in with
  their executable, parity and `dNN_` tests (DESIGN §14, D118–D132 and D30 extended;
  COMPATIBILITY.md §3.15 and §5.3, which maps the three TS specs). The live tier has the
  opt-in bulk flow (`AIRS_RS_LIVE_BULK=1`, at most 8 billed prompts, DESIGN §10, TESTING
  §6.8). Against `redtail-prod` it recovered an interrupted job with exactly one new
  submission per pending chunk, a final resume sent nothing, and the TypeScript verdicts
  agreed. The choices of DESIGN §14.20 shipped as written and are confirmed.
- **M6a (0.6, released).** Every `redteam` command except `judge` is in, in the three build
  steps of DESIGN §15.1: the reads, the writes and the scan, then `report|dashboard` (one job
  and the HTML environment report), `targets backup` and `targets restore`, with their
  executable, parity and `dNN_` tests (D133–D159; D11, D52, D54, D116 extended;
  COMPATIBILITY.md §3.16–§3.19, §5.4). The live tier has the red team reads, the
  draft-adapter write flow (`AIRS_RS_LIVE_REDTEAM_WRITES=1`) and the billed scan flow
  (`AIRS_RS_LIVE_REDTEAM_SCAN=1` plus `AIRS_E2E_REDTEAM_TARGET`, one scan), DESIGN §10.
  Against `redtail-prod` the reads agreed on every check (19), and the write flow created,
  updated and deleted one draft adapter per CLI with equal projections and nothing left
  behind; the scan flow was not run for 0.6. An adversarial review found 18 issues, of
  which 14 were confirmed and fixed. The choices of DESIGN §15.22 are confirmed
  (2026-10-18; choice 19 changed D157: a target whose credentials the service withholds
  is stored with a marker and refused by restore). SDK gap G10 (the printing half of D153)
  stayed open in 0.6; it is resolved in 0.7 (below).
- **M6b (0.7, released).** Step 1 of DESIGN §16.1 is in: the eight `runtime dlp` reads
  with their executable, parity and `dNN_` tests (D170, D171, D178 new; D50, D52, D56, D162,
  D174 extended for the reads; COMPATIBILITY.md §3.20, §5.5). Found while building it: scm-sdk's
  response mode keeps an unfamiliar enum value, so a DLP read prints a new `status` where
  TypeScript fails (D178, DESIGN §16.15 choice 28). Step 2 is in: every write
  (`filtering-profiles replace`; pattern, profile and dictionary `create`, `replace`, `patch`,
  the pattern and dictionary deletes, the `profiles delete` refusal) with its executable, parity
  and `dNN_` tests (D160, D161, D163–D169, D172, D173 pinned; D50, D52, D116, D162, D174 extended
  to the writes; COMPATIBILITY.md §3.21). Choice 28 was resolved toward safety: the predefined
  guard also refuses a record whose `type` is outside the tenant-created vocabulary. Step 3 is
  in: `runtime dlp backup` (the Rust `airs-cli-rs.dlp-resources` format) and `runtime dlp restore
  <file>` (the TS planner, the TSG check, one attempt per create, the deterministic reconcile
  suffix), with their executable, parity and `dNN_` tests (D175, D176, D177 pinned; D50, D52,
  D160–D162, D164, D167, D172, D174 extended to backup and restore; COMPATIBILITY.md §3.22, §3.23),
  and the live tier's DLP reads, the DLP write flow (`AIRS_RS_LIVE_DLP_WRITES=1`) and the
  sweeper's DLP kinds with their loopback self-tests. Found while building it: the destination
  inventory carries no keywords (G13), so restore reads the keywords of each same-name
  dictionary per id before planning (DESIGN §16.15 choice 29). 0.7 also moves to scm-sdk
  0.1.1, whose `RequestOptions::without_response_defaults()` resolves G10: the target reads,
  the target write answers and the `targets backup` read print and store only the fields the
  server sent, so D153 is complete (its printing half pinned by `d153_` read and write parity
  tests; the backup's read is the typed `get`, `red_team.targets.get` in the `--debug` log).
  Against `redtail-prod` the DLP reads agreed on every check (13), the red team reads still
  agreed on all 19 with scm-sdk 0.1.1, and the DLP write flow (one pattern and one dictionary
  per CLI, plus the Rust restore of prefixed copies) passed 30 checks, creating and deleting 6
  resources with nothing left behind. Data profiles are not written live (G11: the API cannot
  delete one). An adversarial review confirmed 6 issues, all fixed. The choices of DESIGN
  §16.15 are confirmed (2026-10-18).
- **M6c.** TypeScript's `model-security install --dry-run` prints the PyPI index URL,
  which contains a credential. The Rust port redacts it (§12).
- **M6c (0.8, released).** DESIGN §17 is the contract, built in the three steps of §17.1 (the
  reads; the writes; `pypi-auth`, `install` with `--fetch-uv` and the live tier), with D179–D203 (D11,
  D50, D52, D64 and D116 extended) and the decided choices of §17.18. scm-sdk 0.1.1 serves every
  command once the `model-security` feature is enabled; G18 (no `scan_origin` list filter) is blocked
  on scm-sdk and worked around with an exact local filter. `--fetch-uv` pins uv 0.12.17 with the
  official SHA-256 values; its archive layout (`uv-<target>/uv`) was confirmed before the tag by
  downloading the four pinned archives and extracting each with the CLI's reader. Scans created by the live scan flow cannot be deleted (no
  API), so that flow leaves one scan per run.
- **M6c step 1 (reads) built.** Every read of §17.1 step 1 (`groups`, `rules`,
  `rule-instances`, `scans` with their sub-reads, `models`, `labels keys|values`), the
  `model-security` feature of scm-sdk 0.1.1 (no lock-file change), the shared page loop
  `src/walk.rs` (moved from `src/redteam/walk.rs`, with the `count_capped` latch) and the
  fixture's model-security read routes, pinned by `tests/modelsec_reads.rs` and
  `tests/parity_modelsec_reads.rs` (D180–D184, D189–D193; D11, D50, D52, D64 extended).
- **M6c step 2 (writes) built.** `groups create|update|delete|rm`, `rule-instances update`,
  `scans create` and `labels add|set|delete|rm` (DESIGN §17.8–§17.11): one attempt per write,
  the confirmations, the delete verification, the config and argument checks before the tenant,
  and the fixture's write routes, pinned by `tests/modelsec_writes.rs` and
  `tests/parity_modelsec_writes.rs` (D179, D185–D188; the write halves of D180, D184, D189–D191,
  D193; D11, D50, D52, D64 and D116 extended). No dependency or lock-file change; nothing blocked
  on scm-sdk.
- **M6c step 3 (`pypi-auth`, `install`, live) built.** `pypi-auth` with `--url-file`, `install` (uv
  only, `--dry-run`, `--fetch-uv` with the pinned, checksum-verified uv 0.12.17, the credential in
  `uv add`'s environment only, the persistence check, the signal forwarded to uv's process group),
  the fake uv and the loopback uv archive, and the live tier's model-security reads with the
  Rust-only `pypi-auth` check, the group write flow, the billed scan flow (budget 1) and the
  sweeper's groups, pinned by `tests/modelsec_install.rs`, `tests/parity_modelsec_install.rs` and
  the `tests/live.rs` self-tests (D194–D201; the step-3 halves of D11, D50, D52, D64, D189, D193).
  `flate2` (already in the lock file) is now a direct, exact-pinned dependency of the CLI and the
  harness; the lock file gained only those two edges. Nothing of step 3 is blocked on scm-sdk.
- **M6c review and release.** An adversarial review (write safety, credential and file safety,
  parity) confirmed 11 issues, all fixed. The fixes include `uv add --no-build` (D203), a workspace
  check over the typed and canonical ancestors, the scan flow needing the write opt-in too, and
  D202 recorded. The choices of DESIGN §17.18 were decided under the standing autonomy (2026-10-18).
  Against `redtail-prod` the service account has no model-security access: both CLIs get HTTP 403
  on `groups list`, and `pypi/authenticate` answers 401, 403 or 404. So the model-security reads,
  the Rust `pypi-auth` check and the group write flow ran on loopback only. The billed scan flow
  was not run, and `install` is never live. The base comparison (20 checks), the red team reads (19)
  and the DLP reads (13) all still agree on the 0.8 tree. scm-sdk 0.2.0 is not adopted (G25, DESIGN
  §15): its vendor 0.93.0 response schemas enforce string formats (UUID, RFC 3339), required
  fields and non-null types that the TypeScript SDK leaves open. 0.9.1 adopts scm-sdk 0.2.2, which closes G25 (DESIGN §15.23).
- **M6d step 1 (reads) built.** `runtime api-keys list`, `customer-apps list|get|consumption`, `deployment-profiles list`,
  the `scan-logs query` stub and every `dashboard` and `sessions` read (DESIGN §18.4, §18.7, §18.9–§18.13), with
  `ManagementClients::build_for` and its dashboard plane, `walk_with` in `src/walk.rs`, the views and layouts of
  `src/runtime/`, the `api_key` withholding, and the fixture's runtime and dashboard routes (`Route::Runtime`), pinned by
  `tests/runtime_extras_reads.rs`, `tests/runtime_dashboard.rs`, `tests/parity_runtime_extras.rs` and
  `tests/parity_runtime_dashboard.rs` (D205, D207, D209–D212, D219, D220, D222, D224, D225; the read halves of D11, D12,
  D14, D35, D37, D50, D52, D64, D73; DESIGN §18.20 choices 41–46, 73 and 74: each read's request options are built when
  it is sent, so an all-apps `consumption` on a large or slow tenant gives every app read its own budget, and the
  empty-overview phrase is printed only for an empty overview). No dependency or lock-file change; nothing blocked on
  scm-sdk.
- **M6d step 2 (writes and the report) built.** `runtime api-keys create|regenerate|delete|rm`, `customer-apps
  update|delete|rm` and `runtime report` (DESIGN §18.5, §18.6, §18.8, §18.14), with the `--key-file` reservation
  (`files::Reservation`), the G26 raw key POSTs, the confirmations, the one-attempt writes and their outcome lines, the
  runtime collector and HTML renderer (`src/report/runtime.rs`, `runtime_html.rs`), and the fixture's write routes,
  pinned by `tests/runtime_extras_writes.rs`, `tests/runtime_report.rs`, `tests/parity_runtime_extras_writes.rs` and
  `tests/parity_runtime_report.rs` (D204, D206, D208, D213, D214, D221, D223; the write halves of D207, D220, D222, D224;
  D11, D52, D64, D95, D155, D156 extended; DESIGN §18.20 choices 47–56). No dependency or lock-file change; nothing
  blocked on scm-sdk (G26 amended: the SDK's `named` is crate-private, so the raw POSTs are `custom.request` in the debug
  log).
- **M6d step 3 (profile transfer and the live tier) built.** `runtime profiles backup [profile]` and `runtime profiles
  restore <file>` (DESIGN §18.15, §18.16): the Rust format `airs-cli-rs.runtime-profiles` v1 and its strict reader
  (TypeScript backups refused), the TypeScript planner and executor rules and `compareRuntimePolicies`
  (`src/runtime/transfer/`), the TSG checks, one attempt per write with the outcome-unknown stop, and the D218 refusal
  of absent collections; the live tier's `live_runtime_reads` (the M6d reads, both CLIs, Rust only where TypeScript could
  print a key) and `live_profile_transfer` (`AIRS_RS_LIVE_PROFILE_TRANSFER=1`: one run-created topic and profile backed
  up, deleted, restored, verified and deleted, at most four resources, journalled), with their loopback self-tests.
  Pinned by `tests/runtime_transfer.rs`, `tests/parity_runtime_transfer.rs`, the `src/runtime/transfer/` unit tests and
  `tests/live.rs` (D215–D218; the transfer halves of D50, D73, D156, D220, D224; DESIGN §18.20 choices 57–66). No
  dependency or lock-file change; nothing blocked on scm-sdk. The live tier has not been run against a tenant.
- **M6d review fixes.** The transfer commands run the tenant checks before the client is built (choice 77, pinned by
  `the_tsg_checks_come_before_the_client`), the restore's stop texts are typed refusals (choice 78), `api-keys list` and
  `customer-apps list` share `runtime::list::OffsetList` (choice 79), and the generic helpers the runtime commands
  borrowed from the red team and DLP modules moved to `segment.rs`, `json.rs`, `files.rs` and `pagination.rs` (choice
  80). No observable change beyond choice 77.
- **M6d scoring and release.** From 0.9 each build step is scored out of 50 by an independent reviewer
  (bar 42, every dimension at least 8), and the milestone as a whole (bar 46): reads 41 → 46, writes
  and report 42 → 44, transfer and live tier 42, milestone 44 → 47. Against `redtail-prod` (2026-10-19)
  every live tier agrees: base 20, red team 19, DLP 13, runtime reads 21 (`customer-apps get` is
  HTTP 403 on both CLIs and recorded as unavailable), profile transfer 15 (5 created, 5 deleted).
  The API-key and customer-app writes are never live.
- **M6e (0.10).** Built in four chunks (DESIGN §19.1): the workspace, scope and telemetry reads; the
  inventory and admin-guardrail reads; every write (one attempt, confirmations, `--secret-output`); and chunk 4,
  `gateway inference chat|responses|embeddings` (the tenant's runtime key only, never `PANW_AI_GW_INFERENCE_*`, D240;
  model text through `terminal_safe_lines`, a split secret withheld, 130/143 with the billing context, D241), `gateway
  report|dashboard` (the 25 sources through the shared environment renderer, now parameterised by a `Profile`; the red
  team and runtime reports byte-identical; D249), the doctor's eighth check `Gateway API` (one shared token, one
  attempt, D250) and the live tier's `live_gateway_reads` (read-only on both CLIs, Rust only where TypeScript could
  print a credential; the session guard refuses every gateway write and inference in every live flow). Pinned by
  `tests/gateway_{reads,inventory,writes,inference,report}.rs`, their `parity_*` files, `tests/doctor.rs::gateway_api`,
  `tests/parity_doctor.rs` and the `tests/live.rs` self-tests (DESIGN §19.20 choices 1–188; 119–130 from the milestone reviews, 131–143 from its final review rounds: the audit reveal paths, the SDK path reading, the 0.2.2 re-measure, the credential members, `sdk_options`, the header maps and the registration of header values, the shared outcome lines, the generic names inside an audit field, the bare token of a credential header, and the hints the outcome lines carry; 144–186 from the withholding review rounds: the placeholder and the registration filter, JSON Schema property maps, camelCase and worded credential names, pairs read as headers, numbers and text-free values, the generated and real-world corpus, the scheme, header and key=value text rules, the rules as one table in a module of the CLI that the live tier compiles, the live table notes, and two confirmations and the one-time credential save of the writes; 187 from the live run and the final milestone review: the names whose last words describe a credential, numbers and empty strings printed under every name, a credential name's word-shaped value withheld in place only, `signature`, `passcode` and `otp`, the linear audit read and `SecretSet`, and the checker in test builds only; 188 from the release's independent review: the key-name lists printed, the state, mode and grant-type words never registered, and a schema's empty instance values printed). No dependency change beyond
  the `gateway` feature (the lock file gained only the `scm-sdk → getrandom` edge); nothing blocked on scm-sdk. The live
  gateway reads were run against a tenant on 2026-10-19 and agree with TypeScript (40 checks, 0 mismatched),
  with fixes from those runs: the guardrail catalog's over-withholding (choice 146), the D35 key order of JSON cells
  (choice 156), and the workspace `settings` that printed `[withheld]` for counts and lifetimes (choice 187).
- **M6f (0.11).** Built in two chunks (DESIGN §20.1): the clients and the four reads (`scans list|ls`,
  `scans vulnerabilities [--include-content]`, `stats`, `rules list|ls`; D308, D313–D321, D323), then chunk 2:
  `supplychain report` (three sources through the shared environment renderer with the new `SUPPLYCHAIN` profile; the
  red team, runtime and gateway reports byte-identical; the default title `Supply chain environment report`, the file
  `airs-supplychain-report-…`, the failure line `Supply chain report failed …`, zoned creation times; D309–D311, D322,
  D323), the doctor's management-credential line naming `supplychain` commands (D312), and the live tier's
  `live_supplychain_reads` (reads only, one page each, `--include-content` Rust only and in memory, the TypeScript
  `--include-content` refused by the session guard; a tenant without AI Supply Chain access records `supplychain
  unavailable` and passes). Pinned by `tests/supplychain_{reads,report}.rs`, their `parity_*` files,
  `tests/doctor.rs`, `tests/parity_doctor.rs` and the `tests/live.rs` self-tests (DESIGN §20.17 choices 1–90; 64–76
  from the first live run: G71, the scan list read raw through a tolerant projection because the service sends `null`
  where both SDK contracts require a scan member, D324, D325, §20.16; 55–63 from the milestone reviews: the JWT-shape check of the content stdout outside its finding rows, a difference whose
  re-read is unavailable kept as a mismatch, a content run refused after the plain read a mismatch, D318 tolerated in
  its shape only, the SDK's maximum validation bounds beside the 32 MiB bound, the one re-read of the content run and
  of the report pair, the walk's per-page check given the requested page size, and the shared supplychain helpers and
  `MAX_SAFE_INTEGER`; 77–81 from the milestone gate review: the raw scan read checked against the G66 validation
  bounds (depth 64, 1,000,000 values, 32 MiB), the rows committed through `Output::commit_data` before the D319 line,
  `walk::total_of` called directly, `withhold` and `json_text` moved to `secrets.rs` and `json.rs`, and the list
  options in TypeScript's help order; 82–89 from the post-gate review: the engine's own query bound, an empty 2xx
  scan body as the request line, batch membership from either signal, G71 in the live tier only with Rust rows and
  TypeScript refusing both reads, the report's table titles compared as a multiset, the validation bounds computed
  once per command, the `debug::named` pin constraint and its loopback test, and one `MAX_SAFE_INTEGER`). No
  dependency change beyond the `agentguard` feature (the lock file does not change); nothing blocked on scm-sdk (G71's
  contract defect is worked around in the CLI; its fix is requested from the SDK's maintainer). The first live run
  agreed on `stats`, `rules list` and the report's other sources and found G71; the live flow passes a scan read that
  TypeScript refuses with its status-less request line on both reads while airs-cli-rs prints its rows.
- **The 0.9.0 change set.** The release commit stages the M6d files only: the tracked changes and the new files under
  `src/` (`src/runtime/`, `src/segment.rs`, `src/test_support.rs`, `src/report/runtime*.rs`, `src/commands/{runtime_*,
  profile_backup,profile_restore,report_command,write_common}.rs`), `crates/airs-test-harness/` (`fixtures_runtime.rs`,
  `server/runtime.rs`, `live/runtime.rs`, `live/profile_transfer.rs`), `tests/{runtime_*,parity_runtime_*}.rs` and the
  three M6d research specs (`docs/research/ts-runtime-extras.md`, `ts-runtime-dashboard-report.md`,
  `rs-sdk-runtime-extras.md`). The gateway and supplychain research (`docs/research/rs-sdk-gateway.md`,
  `ts-gateway.md`, `ts-gateway-report.md`, `rs-sdk-supplychain.md`, `ts-supplychain.md`) belongs to 0.10 and 0.11 and
  is committed with those milestones, not with 0.9.0. The `gateway*` and `supplychain*` tenant keys are not M6d work:
  they shipped in 0.3.0 with the TOML tenant schema (M4t, the `tenant import` renames of DESIGN §3.3 and D70–D90); 0.9
  changes no tenant key.
- **1.0 chunk 1 (the rename).** The executable is `airs` (`[[bin]] airs`, `default-run = "airs"`) and the second
  `[[bin]] airs-cli-rs` includes the same `src/main.rs`, so `cargo install` and the source build provide both names;
  clap's `name` and `bin_name` are `store::PROGRAM = "airs"`, so the start name never reaches the output. Every usage
  line, example, hint, remedy and Rust-only message names `airs` (D73 extended, D326 new); the four file format names
  `airs-cli-rs.*`, the directories, environment variables and tenant keys are unchanged (DESIGN §21.3). Tests run
  `CARGO_BIN_EXE_airs`; `tests/program_name.rs` scans the source, the current documents and every command's help.
  Chunk 2 (tarballs with the link, the Homebrew formula and its script, RELEASING, README Install) follows before the
  1.0 release.
- **1.0 chunk 2 (distribution).** `scripts/release.sh` builds only `airs` (`--bin airs`), signs it (`Identifier=airs`)
  and packages through the new `scripts/package-release.sh`: `airs-cli-<version>-<target>.tar.gz` with `airs`, the
  relative `airs-cli-rs` link, `README.md` and `LICENSE`, every tarball listed and extracted again to check it, and on
  a real run the packaged signature verified. The formula template `packaging/homebrew/airs-cli.rb.in` builds from
  the crates.io crate (the Forgejo assets need a login); `scripts/homebrew-formula.sh` renders it from crates.io's
  checksum, confirmed on the downloaded `.crate`, or offline from `--checksum` or a saved answer. The tap
  `cdot/homebrew-tap`, the crates.io publish and the tap commit are the orchestrator's (RELEASING §2, §6). Tests:
  `tests/homebrew_formula.rs`, `tests/release_packaging.rs`; RELEASING joined the docs scan. DESIGN §21.14 choices
  41–54.
- **1.0 scm-sdk 0.2.3.** The pin moves to `=0.2.3` with the same features (only the `scm-sdk` lock entry changes):
  the red team management 0.86.0 update. `targets create|update|probe|restore` send `VERTEX_AI`, `N8N`, `REALTIME`,
  `client_session` and a Vertex AI config within its request patterns, which TypeScript refuses (D328); targets
  with a Vertex AI or `client_session` config are read, printed redacted and backed up where TypeScript and 0.11
  refuse the answer, except that a Vertex AI target whose answer masks its `project_id`, `location` or `engine_id`
  is a failed row naming the masked fields and the remedy (exit 1), and a `VERTEX_AI`, `N8N` or `REALTIME` target
  that 0.11 read but recorded as a backup's create-schema failed row (exit 1) is stored (D329); the Vertex AI
  identifiers are redacted but never registered, so the endpoint, name and description that hold them print, and a
  secret-named key registers under any key; `audio_curl` is redacted like `curl` (D327); the 0.86.0 fields print in
  contract order (D35 extended). No other output changes (the full parity suite); the user agent is
  `PAN-AIRS/0.2.3-rust-sdk`; no SDK gap opened. Tests: the contract guard, the redaction and backup unit tests,
  `management_0_86` modules in `tests/redteam_{reads,writes,reports}.rs`, `d327_`–`d329_` parity tests, the live
  redaction check. DESIGN §15.24 choices 1–15.
- **8.0 the TypeScript 7.3.0 port (Part B).** The reference moves to tag `v7.3.0` (`7a54e46`, TS SDK 0.35.0), the final
  move (TESTING §3.4); against it the unchanged tree failed 22 tests (the version, the profile and topic writes, the
  profile detail), each classified into a port, a reworded or retired row, or a new row. scm-sdk 0.2.4's overlay was
  first adopted from its exact Forgejo commit `4825616`, then, once published, as `=0.2.4` from crates.io (DESIGN
  §15.25: the published sources are that commit's; the features are unchanged; `Cargo.lock` changes `scm-sdk` and, per
  the SDK's advisory, `rustls-webpki` and `webpki-root-certs`; `cargo audit` is clean). The port
  (DESIGN §13.17): `--direction`, `--ai-profile-index` and the 7.3.0 flags on `profiles create|update` and `topics
  apply|revert`; TS 7.3.0's builder, recursive merge, layout refusals (exit 2), `--config` rules, status lines,
  banner-less detail and tenant `defaultOutput`; every profile body validated with the TypeScript 0.35.0 request
  root; the profile views (`dlpTenantId`, `DLP tenant`, `Protection locations`, TS 0.35.0 key order), the runtime
  report's `Protection configurations` column, and the transfer walks over every direction; the 0.2.4 fields keep
  omission, explicit `false`, string DLP tenant ids and unknown directions, and an unexpected value in an answer is
  printed and backed up as sent (D334). Rows: D332, D333, D334 new; D117
  retired; D26, D50, D93, D94, D96, D98–D103, D106, D115, D116 reworded. Tests: `tests/parity_runtime_directional.rs`
  (every scenario of 7.3.0's directional integration spec on both CLIs, on its fixture, the 0.2.4 field cases, plus
  `d332_`–`d334_`), the `sdk_0_2_4` module of `tests/runtime_writes.rs`, the directional cases in
  `parity_runtime_writes`, `parity_runtime_topics`, `parity_runtime_report`, `parity_runtime_transfer` and
  `runtime_transfer`, and the `src/profile_policy.rs`, `src/policy.rs` and `src/views.rs` unit tests. The bounded
  live run per direction (B7) is done (below, with D335); the adversarial review's findings are fixed (below, with D336).
- **8.0 the scm-sdk 0.2.4 gap closure.** A second, more detailed adoption brief was audited item by item against PR #5.
  Gaps closed (DESIGN §15.25 point 4 and "Cumulative migrations", §13.17, §7.5): every profile read (`list`, `list
  --all`, `get` and its walks, the update and delete lookups, the `topics apply|revert` profile reads, the `cleanup`
  listings) and every profile write now runs without the SDK's response defaults (no output change: only
  `ai_profiles: []` is reachable at 0.2.4), pinned by an options test and a call-site guard; a restore re-checks each
  mapped body with the 0.35.0 request root. New regressions: `tests/management.rs::sdk_0_2_4_pages` and
  `runtime_transfer::backup::directional_records_are_stored_losslessly_on_every_page` (directional records on page 2
  printed and backed up as sent), the flag update's debug operation names, `targets error-logs --skip` as a usage
  error, every 0.2.3 `profiling_status` accepted and never sent, unknown 0.86.0 union shapes refused on both CLIs (a
  parity test; a failed backup row), the `api.apps` defaults, `serde_json` without `preserve_order`, and flag creation
  of each direction. Kept by contract: the get/list/create/update JSON is TypeScript's §7.5 projection (unknown
  top-level keys dropped there; `profiles backup` is lossless). No D-row added. Reviews (independent reviewer
  agents): the plan, ACCEPT WITH CHANGES (nine minor and nit findings), then ACCEPT after revisions; the completed
  PR (`origin/main...HEAD`), round 1: implementation 9, code quality 9, supportability 8, architecture 9, delivery 8
  = 43/50, no finding above minor (fixed: the cleanup-deadline and topic-PUT wording, a call-site guard test, this
  record, a vacuous request check in the `--skip` test, the restore re-check described as defence in depth; left as
  a nit: the runtime report's own directional predicate, `src/report/runtime.rs:220-237`); round 2: 9, 9, 9, 9, 8 =
  44/50, no finding above minor (two nits fixed: a doc comment's place and the guard's stated scope). Every
  dimension is at least 8 and both rounds clear the build-step bar of 42.
- **8.0 B7, the live run of directional profiles (2026-10-22, `redtail-prod`).** Run with the release binary
  against the CLI's own tenant store, every resource named `airs-rs-live-<run id>-…`, each write read back with
  `profiles get --output json` and diffed per direction. The step table, the bisection and the request ids are in
  [research/live-directional-profiles.md](research/live-directional-profiles.md). Found: the service refuses
  TypeScript 7.3.0's one-direction flag create (HTTP 400, for every direction) because it requires all four
  direction keys. Fixed test-first as **D335**: a flag create writes the other three directions as `{}`
  (`d335_flag_creation_writes_every_known_direction`, the `src/policy.rs` and `runtime_writes` tests updated).
  Then accepted for each of `prompt`, `response`, `tool-call` and `tool-response`: flag create (with D335), a
  four-direction `--config` create, a protection edit per direction and a shared-settings edit (each changing
  only its location), a topic applied and reverted per direction, backup, delete of every revision, restore
  (policy identical to the backup) and `--on-conflict verify`. Service rules recorded, not checked locally (a 400,
  exit 1): all-empty directions refused, `prompt-injection` refused in `response`, `toxic-content` levels `block`
  or `allow` only (the toxic `alert` refusal seen once). `dlp_tenant_id`: no `get` or `list` answer carries it; a write
  answer echoes the value sent, or `"none"` when the body omits it. The answer reports `none`; the effect on the stored
  binding is unknown (an open question for the service owner, with the distinguishing test, in the evidence file).
  Not exercised live: several AI entries, DLP blocks inside a direction, the report column. Cleanup: 61 deletes
  (57 profile revisions, 4 topics); the final listings equal the preflight's (35 profile revisions, 5 topics, zero
  `airs-rs-` resources). Directional profiles are no longer marked experimental.
  **Live test rules.** The maintainer's release brief set the `airs-rs-live-` prefix and authorised B7. The departures
  from the rules above: the `live_cleanup` sweeper does not recognise that prefix; 11 named resources against the cap
  of 10, plus about 57 profile revisions; no `target/live/ledger.jsonl` ledger (a step log outside the repository).
  The maintainer waived them for B7, given that the final listing matched the preflight's.
  **Follow-up (open):** a Rust-only directional step in `live_write_flow` (`tests/live.rs`, recorded under D93), so a
  later run repeats B7 within the live tier's prefix, ledger and cap. Not implemented; it must not run live before
  it is reviewed.
- **8.0 the adversarial review of PR #5 (at `24be812`).** Findings fixed offline: the `dlp_tenant_id` claim on writes
  reworded to "the answer reports `none`; the effect on the stored binding is unknown" (DESIGN §13.17, the evidence
  file) and disclosed as a known limitation for every write path (README, the docs site, MIGRATION, CHANGELOG); the
  docs' `toxic-content … alert` examples on directional profiles replaced with `block`/`allow`; **D336**: a
  directional flag create whose flags fill no direction (none, or shared settings only) is refused before OAuth
  (exit 2) with a text naming the service rule (test-first: `d336_a_directional_flag_create_without_a_protection_is_refused`,
  `src/policy.rs::tests::only_protection_flags_fill_a_direction`, the `runtime_writes` usage test); one known-limitations
  list across README, the docs site, CHANGELOG and MIGRATION (directional profiles supported, for single-entry
  profiles); the `DLP tenant` line hedged; the B7 live-rule departures and their waiver recorded (above); the parity
  test renamed to `an_empty_directional_flag_creation_is_refused`, the D335 test checking the method and path.

## Signal Brief implementation (unreleased)

Authorized scope: replace runtime report output and repair scan-log discovery,
with bounded report snippets, safe normalized evidence and offline agent review.
The phased plan is docs/SIGNAL_BRIEF_IMPLEMENTATION_PLAN.md; independent phase
scores are recorded in docs/SIGNAL_BRIEF_PHASE_REVIEWS.md. Every phase 0–6 passed
independent Codex review at 9/10. Implementation and final validation are complete:
3,674 tests passed with no failures or parity skips; formatting, Clippy, MSRV and
site build passed. Outcomes are recorded in
docs/SIGNAL_BRIEF_VALIDATION.md. This does not declare the release milestone complete.

The implementation replaces default runtime-report HTML (D340), adds offline
replay/private bundles and review import (D337, D341), and repairs discovery plus
direct scanner lookups (D338–D339). The shared SDK detail-token telemetry supports
7/30/60-day windows; daily briefs explicitly show this gap. No release, push,
agent-run live collection or SDK checkout changes are in scope.

Release-candidate preparation: the unreleased Signal Brief is versioned
8.1.0-rc.1, targeting stable 8.1.0 after user-run validation. Candidate notes are in
docs/RELEASE-NOTES/8.1.0-rc.1.md; build and verification outcomes are tracked in
docs/SIGNAL_BRIEF_TEST_BUILDS.md and docs/SIGNAL_BRIEF_VALIDATION.md. No release has
been published by this implementation task.
