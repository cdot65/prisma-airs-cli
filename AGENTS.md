# airs-cli-rs contributor contract

`airs-cli-rs` is a native Rust port of the TypeScript `airs-cli`, and it replaces it:
this repository, `cdot/prisma-airs-cli`, held the TypeScript CLI until 7.x, and from 8.0
it holds the Rust CLI (crate `airs-cli`, command `airs`). The TypeScript reference is
this repository's own history at the commit pinned in `scripts/build-reference-cli.sh`
(the TypeScript sources are no longer in the tree; they stay in the history and in every
`v7.*` tag, which must never be deleted); from 8.0 that is the final TypeScript release,
tag `v7.3.0` (docs/TESTING.md §3.4). It is built on the `scm-sdk` crate, imported as `scm`,
at the exact pin `=0.2.4` from crates.io (the TypeScript 0.35.0 overlay, DESIGN §15.25). The goal is full parity without the TypeScript
CLI: the pinned reference commit is a frozen spec, not a companion. The scope, the drops
and renames, and the milestone order are recorded in
[docs/MILESTONES.md](docs/MILESTONES.md).

Read [docs/DESIGN.md](docs/DESIGN.md) before changing behaviour. It is the binding
contract: exact strings, exit codes, request shapes and the list of intentional
differences from the TypeScript CLI (§12). If a change alters observable behaviour,
update DESIGN.md in the same change: the section that specifies it and, when the
TypeScript CLI behaves differently, the §12 row. Pin the behaviour with an
executable test, and a §12 difference with a `dNN_`-named test in the matching
`tests/parity_*.rs` that asserts both sides. Then update
[docs/COMPATIBILITY.md](docs/COMPATIBILITY.md), the compatibility checklist that maps
each behaviour and each §12 row to the tests that pin it; `tests/docs_consistency.rs`
fails when a cited test, a §12 row or a `dNN`-named parity test is missing from it.

## Invariants

- **Tenants live in the TOML store (M4t, DESIGN §3).** Commands read only
  `D/config.toml` and `D/tenants/<name>.toml` (`AIRS_CLI_CONFIG_DIR`, else
  `$XDG_CONFIG_HOME/airs-cli`, else `~/.config/airs-cli`) with the §3.8 safety checks.
  Readers never write, create, lock, rename or chmod anything. Writes go only through
  `store::atomic::Transaction` (the `flock` on `D/config.lock`, temp + fsync + rename +
  directory fsync, 0600/0700). The TypeScript registry and configs are read only by
  `tenant import` (`src/tenant/ts.rs`, `src/tenant/import.rs`), which never modifies them or opens their
  `*.lock` files; other code only checks whether the registry path exists (the import
  hint). Hints that ask users to change tenants name `airs` (`store::PROGRAM`,
  D73).
- **Tenant files are the only configuration source.** Never read credentials or
  endpoints from the environment. Never call `EnvironmentSnapshot::capture`,
  `init_from_environment`, or `resolve_oauth_config` with a captured snapshot.
- **Two credential planes.** Management commands use the shared `mgmt*` OAuth client
  credentials; scanning uses `airsApiKey`/`airsApiToken`. Neither plane may send the
  other's credentials.
- **Streams.** stdout carries data only. stderr carries status lines (suppressed by
  `--quiet`) and errors.
- **Exit codes.**
  - 0: success.
  - 1: operational failure.
  - 2: usage error, including every argument-parse error. Usage validation runs
    before the tenant is read.
  - 130 / 143: interrupted.
- **Secret safety.** No output path may print secrets, tokens, raw response bodies or
  URL queries. Build error text from SDK error metadata only (DESIGN §6). Server text
  that is printed (scan reasons, IDs, categories, `result`) is checked against the
  invocation's `SecretSet`, which records every OAuth lease the SDK obtains. Printed
  server or file text never carries control characters (`output::terminal_safe`).
  Never format SDK models with `{:?}` for users.
- **Fail closed.** Unknown, failed or timed-out scan outcomes are never shown as allow.
  Incomplete traversals are never reported as complete or as not-found.
- Project code forbids `unsafe`.

## Working in this repository

- **Current acceptance threshold (user instruction, 2026-11-06): 9.0/10 or higher.** (Was 9.1 from 2026-11-04.)
  Apply this threshold to outstanding implementation and requalification reviews,
  including runtime-history phases B3–B6. Preserve historical review scores as
  recorded; acceptance at the new threshold requires a fresh independent review
  of the actual source and verification evidence. Required checks must pass, and
  unresolved findings must be corrected before the work is reported complete.
  [Independent B4/B5 review notes](docs/RUNTIME_HISTORY_B4_B5_PRE_REVIEW.md)
  record source-grounded acceptance probes and B6 preparation requirements;
  they do not certify unimplemented phases.

- Rust tools live in `~/.cargo/bin`; prefix shell commands with
  `export PATH="$HOME/.cargo/bin:$PATH";` when it is not on PATH.
- Checks before finishing a change:

  ```sh
  cargo fmt --all --check
  cargo clippy --workspace --all-targets -- -D warnings
  AIRS_REQUIRE_PARITY=1 cargo test --workspace
  ```

  Parity tests need the TypeScript reference build (`scripts/build-reference-cli.sh`)
  and node 24 on `PATH` (or `AIRS_TS_NODE`). A missing, off-pin or modified reference
  fails the parity tests; `AIRS_ALLOW_PARITY_SKIP=1` skips them with a visible
  `PARITY SKIPPED` notice, which is not a finished check.
- Tests never contact a non-loopback host. Fixture tenants point every endpoint at the
  in-process server from `crates/airs-test-harness`, and planned redirects must stay on
  loopback. The runner fails any run that changes a sandbox file outside `tmp/` (the
  Rust store under `store/` included), and refuses to start a run whose path variables,
  absolute-path arguments or path-flag values (every occurrence) point outside the
  sandbox, and an airs-cli-rs `tenant import` run whose registry or configs lie outside
  it. Every sandbox provisions each
  TypeScript tenant into the Rust store too (DESIGN §10), so tests never touch the
  real `~/.config/airs-cli`. `tenant` commands build no API client, so the runner
  skips the loopback endpoint checks for them (a created tenant holds only
  credentials), never the path guards. Prompts are tested on a pty
  (`airs_test_harness::spawn_pty`) and the store lock with `airs_test_harness::lock_holder`.
- The live tier (`tests/live.rs`, DESIGN §10) runs only with
  `AIRS_RS_LIVE=1 AIRS_E2E_TENANT=<name>`, and only the user runs it. It must keep
  using an isolated copy of the one registry entry, imported with
  `airs tenant import` into a private 0700 store under `target/live/` that is
  removed when the flow ends (also on failure and panic), a cleared environment, the
  scan budget, sanitized summaries and the fingerprint check that the real tenant files
  are unchanged. Never read or print real credential values; its self-tests use
  `LiveTarget::loopback`, which takes a harness `Sandbox` and refuses, before reading,
  any registry or config outside it, against the fixture server, with their private
  stores under the sandbox's `tmp/live-stores/`, never the shared `target/live/`. Keep the scan series
  stopping at the first scan that does not complete, so a failing tenant is not sent
  every planned scan.
- Never check out the TypeScript reference in the working tree, rewrite the history it
  lives in, or delete a `v7.*` tag: the parity harness builds the reference from that
  history into `.reference/` (docs/TESTING.md §3). Do not modify the SDK checkout.

## Issue tracking: BonBon

This project tracks work as Cards on its own BonBon Board, **Prisma AIRS CLI**
(`01a1ab75-35eb-751c-8d38-45f09771aa3b`). Every Card carries the Label `prisma-airs-cli`,
one triage Label and at least one kind Label (see `docs/agents/triage-labels.md`), and its
title starts with `[AIRS CLI] `. Never put this project's work on any other Board.

- **Claude Code is the only BonBon interface** (the `bonbon` skill and MCP server; the
  `bonbon-owner-api` skill as a fallback). Other agents never connect to BonBon. They write
  the Card changes they need into their handoff, and a Claude Code session applies them.
- In Claude Code: `whoami` first, then search this Board before creating anything. Read a
  Card fully before working it. Claim before executing, heartbeat at least every 20 minutes,
  and finish with `complete_card` and a Result.
- Only the Owner validates: never mark work DONE or VALIDATED, and preserve other agents'
  Claims.
- Keep secrets, device identifiers and personal or household data out of Cards.
- Board activity alone authorizes no implementation, deploy, commit, push or message.

Details: `docs/agents/issue-tracker.md`.
