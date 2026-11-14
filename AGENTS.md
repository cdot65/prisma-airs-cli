# airs-cli-rs contributor contract

`airs-cli-rs` is the Rust CLI (crate `airs-cli`, command `airs`). It replaced the
TypeScript `airs-cli`, which this repository held until 7.x; from 8.0 the repository holds
only the Rust CLI, and nothing is compared against the TypeScript CLI any more (the
TypeScript parity tests and reference build were removed on 2026-11-08). It is built on
the `scm-sdk` crate, imported as `scm`, at the exact pin `=0.2.6` from crates.io
(DESIGN §15.25). The scope and the milestone order are recorded in
[docs/MILESTONES.md](docs/MILESTONES.md).

Read [docs/DESIGN.md](docs/DESIGN.md) before changing behaviour. It is the binding
contract: exact strings, exit codes and request shapes; §12 is the historical record of
where the Rust CLI deliberately differs from the TypeScript CLI it replaced. If a change
alters observable behaviour, update DESIGN.md in the same change and pin the behaviour
with an executable test. Then update [docs/COMPATIBILITY.md](docs/COMPATIBILITY.md), the
checklist that maps each behaviour to the tests that pin it; `tests/docs_consistency.rs`
fails when a cited test is missing.

## Invariants

- **Tenants live in the TOML store (M4t, DESIGN §3).** Commands read only
  `D/config.toml` and `D/tenants/<name>.toml` (`AIRS_CLI_CONFIG_DIR`, else
  `$XDG_CONFIG_HOME/airs-cli`, else `~/.config/airs-cli`) with the §3.8 safety checks.
  Readers never write, create, lock, rename or chmod anything. Writes go only through
  `store::atomic::Transaction` (the `flock` on `D/config.lock`, temp + fsync + rename +
  directory fsync, 0600/0700). Hints that ask users to change tenants name `airs` (`store::PROGRAM`,
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
  cargo test --workspace
  ```

  The npm launcher tests need node on `PATH` (or `AIRS_TEST_NODE`).
- Tests never contact a non-loopback host. Fixture tenants point every endpoint at the
  in-process server from `crates/airs-test-harness`, and planned redirects must stay on
  loopback. The runner fails any run that changes a sandbox file outside `tmp/` (the
  Rust store under `store/` included), and refuses to start a run whose path variables,
  absolute-path arguments or path-flag values (every occurrence) point outside the
  sandbox. Every sandbox provisions each
  TypeScript tenant into the Rust store too (DESIGN §10), so tests never touch the
  real `~/.config/airs-cli`. `tenant` commands build no API client, so the runner
  skips the loopback endpoint checks for them (a created tenant holds only
  credentials), never the path guards. Prompts are tested on a pty
  (`airs_test_harness::spawn_pty`) and the store lock with `airs_test_harness::lock_holder`.
- Do not delete a `v7.*` tag (the TypeScript releases' history) or modify the SDK
  checkout.

## Issue tracking: BonBon

This project tracks work as Cards on its own BonBon Board, **Prisma AIRS CLI**
(`01a1ab75-35eb-751c-8d38-45f09771aa3b`). Every Card carries the Label `prisma-airs-cli`,
one triage Label and at least one kind Label (see `docs/agents/triage-labels.md`), and its
title starts with `[Prisma AIRS CLI] `. Never put this project's work on any other Board.

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
