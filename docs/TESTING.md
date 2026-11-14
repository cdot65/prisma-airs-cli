# Testing airs-cli-rs

This guide explains how to run the tests, what the harness guarantees, and how to add
tests. (The TypeScript parity tests, the live comparison tier and the TypeScript
reference build were removed on 2026-11-08.)
[DESIGN.md](DESIGN.md) §10 is the contract for the test architecture; when this guide
and DESIGN disagree, DESIGN wins. [COMPATIBILITY.md](COMPATIBILITY.md) maps each
behaviour and each §12 row to the tests that pin it.

Prefix shell commands with `export PATH="$HOME/.cargo/bin:$PATH";` when cargo is not on
`PATH`.

## 1. Tiers

| Tier | Where | What it runs | Network |
|---|---|---|---|
| 1. Unit | `#[cfg(test)]` modules in `src/` and `crates/airs-test-harness/src/` | Pure logic: registry and config validation, JS number coercion, the environment report, output resolution, error mapping, scan normalisation, signal decisions, pagination bounds. | none |
| 2. Loopback executable | `tests/*.rs` | Spawns the real `airs` binary (`CARGO_BIN_EXE_airs`; the compatibility binary `CARGO_BIN_EXE_airs-cli-rs` only in its own tests, §5.21) against the in-process `FixtureServer` from `crates/airs-test-harness`. | `127.0.0.1` only |

Other suites in the workspace:

- `crates/airs-test-harness/tests/`: self-tests of the harness. `sandbox_runner.rs`
  checks the runner invariants with `/bin/sh` as the child, `server.rs` checks the
  fixture server.
- `tests/docs_consistency.rs`: reads `docs/COMPATIBILITY.md`, `docs/DESIGN.md`,
  `README.md` and `tests/parity_*.rs` and checks that they agree (§7.4): the cited
  tests, the divergence rows, the README's version-neutral `airs --version` example
  (`# prints the version, e.g. X`) against its status line, and no DESIGN §21 rename
  that maps a name to itself. It starts no process and makes no request.
- `tests/common/mod.rs`: helpers shared by test files (`mod common;`), not a test
  target: `design_section_21`, the one rule for where DESIGN §21 starts and ends, used by
  `tests/docs_consistency.rs` and `tests/program_name.rs`.
`cargo test --workspace` runs everything above. Nothing in that run contacts a host other
than `127.0.0.1`.

## 2. Running the tests

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --no-fail-fast    # the required check
```

Narrower runs:

```sh
cargo test --lib                                   # unit tests of the CLI crate
cargo test -p airs-test-harness                    # harness unit and self-tests
cargo test --test scan                             # one executable suite
cargo test --test docs_consistency                 # the documentation cross-checks
```

Prerequisites besides Rust: `bash`, node on `PATH` for the npm launcher tests (or
`AIRS_TEST_NODE`), and `jq` on `PATH`
(`tests/homebrew_formula.rs` fails, naming it, without it: `brew install jq` or
`apt-get install jq`; macOS 15 and later ship `/usr/bin/jq`). `ruby`, when present, also
syntax-checks the rendered Homebrew formula. The packaging tests of
`tests/release_packaging.rs` need macOS (bsdtar); elsewhere only its portable tests run
(§5.22).

`tests/mcp_client.rs::a_frozen_pre_m7_binary_refuses_the_mcp_table` needs a frozen
pre-M7 `airs` named by `AIRS_PRE_M7_BIN` (built by `scripts/build-pre-m7-cli.sh` from the
8.0.0 release commit). Without it the test prints `PRE-M7 SKIPPED` and passes, unless
`AIRS_REQUIRE_PRE_M7=1` is set, which makes it fail. A run that executed it prints
`PRE-M7 RAN`.

## 3. The TypeScript reference (removed)

Removed on 2026-11-08 with the parity tests: nothing is compared against the TypeScript
CLI any more. Its releases remain in the repository's history and `v7.*` tags.

## 4. Environment variables

These are all the variables the tests and the harness read. Each was checked against
the code.

| Variable | Read by | Meaning |
|---|---|---|
| `RUST_TEST_THREADS` | the test harness | CI sets `4` in the `linux` job. The job container has no CPU limit, so the default would be one thread per host core (16 on the dedicated runner) while the `musl` job compiles alongside; the runner is sized for 4 CPUs, and the suite's wall-clock tests need that headroom to be real. Locally the default is fine. |
| `PATH` | `runner.rs` | Passed to every child; also where node is found for the npm launcher tests. |
| `AIRS_TEST_NODE` | `tests/npm_package.rs` | Path to the node executable for the npm launcher tests (default: `node` on `PATH`). |
| `AIRS_UPDATE_CLI_REFERENCE` | `tests/cli_reference.rs` | `1`: rewrite the docs site's command reference (`docs-site/docs/reference/commands/`) instead of checking it (§5.23); `scripts/cli-reference.sh` sets it. Any other value checks. |

`NO_COLOR=1` is always set for children. `CLICOLOR_FORCE` is never passed to them,
because clap would honour it (DESIGN §2.2).

## 5. Safety guarantees

The harness enforces these on every run. A violation fails or panics the test; none
is a warning.

### 5.1 The child process

`run` and `spawn` in `crates/airs-test-harness/src/runner.rs` start each CLI with:

- stdin `/dev/null` (a test can pipe bytes with `RunOptions::stdin`, or keep an empty
  pipe open until the child exits with `RunOptions::stdin_held_open`) and the working
  directory `<sandbox>/cwd`;
- a cleared environment holding only `PATH`, `HOME=<sandbox>/home`,
  `TMPDIR=<sandbox>/tmp`, `PRISMA_AIRS_TENANTS_PATH=<sandbox registry>`,
  `AIRS_CLI_CONFIG_DIR=<sandbox>/store` (the Rust store; TypeScript ignores it),
  `NO_COLOR=1`, `NO_PROXY=127.0.0.1,localhost`, plus what the test adds with
  `RunOptions::env`;
- an escape check before spawning (`escape_violation`): every path variable (`HOME`,
  `TMPDIR`, `AIRS_CLI_CONFIG_DIR`, `XDG_CONFIG_HOME`, `XDG_STATE_HOME`, `XDG_DATA_HOME`,
  `XDG_CACHE_HOME`, `PRISMA_AIRS_TENANTS_PATH`; relative values against the sandbox
  cwd, `~` against `HOME`), every absolute-path argument (`/…`, `--flag=/…`) and every
  value of a path flag (`--from`, `--from-config`, `--config`; each occurrence, relative
  ones against the sandbox cwd) must lie inside the sandbox, and `HOME` may be removed only while `AIRS_CLI_CONFIG_DIR` (or
  `XDG_CONFIG_HOME`), `PRISMA_AIRS_TENANTS_PATH` (or `XDG_STATE_HOME`) and, for
  `--debug`, `XDG_STATE_HOME` are set; otherwise the run panics. So the only store,
  registry and log directory a child can reach is inside its sandbox;
- a timeout (`DEFAULT_TIMEOUT`, 30 s, or `RunOptions::timeout`) after which the child is
  killed and the test panics;
- a required exit status: death by a signal fails the test unless
  `RunOptions::expect_signal` is set.

A fixture executable can briefly return `ETXTBSY` (`Text file busy`) before a
child starts, for example while a fork retains a writable script descriptor.
The runner retries only that pre-exec failure for at most one second within the
existing run timeout. Other spawn failures return immediately, and a started CLI
is never retried. The Linux regression holds and closes a real script writer;
the deadline/error regression also runs on the other supported Unix targets.

### 5.2 Loopback guard

No executable or parity test can reach a host other than `127.0.0.1`:

- Every config written through `Sandbox` must name every key in `ENDPOINT_KEYS` with a
  loopback `http(s)` URL (`localhost`, `127.0.0.0/8`, `[::1]`, no credentials), or with
  a value that makes both CLIs reject the config before building a client. Otherwise
  the write panics, because an omitted endpoint key would fall back to a production
  default. `FixtureConfig::standard` points every key at the fixture server.
- A Rust-only test that proves zero requests with a non-loopback config opts out with
  `Sandbox::write_config_allowing_external_endpoints` or
  `Sandbox::add_fixture_tenant_allowing_external_endpoints`. The runner then refuses to
  start the TypeScript CLI in that sandbox.
- Before every TypeScript run, `Sandbox::assert_safe_for_ts` re-checks each file under
  `configs/` and each config the TypeScript registry lookup would load. It also refuses
  an environment variable whose name contains `ENDPOINT` or ends in `_PROXY` (other
  than `NO_PROXY`), in any case, unless it is empty or a loopback URL, and it refuses
  `NODE_OPTIONS` outright.
- A planned `location` or `content-location` header must be relative or loopback, or
  planning it panics. The TypeScript CLI follows redirects with its credentials (D30).
- `tenant` commands of either CLI never build an API client, so they cannot send a
  request. For a run whose first positional argument is `tenant`
  (`is_tenant_command`), the runner skips the endpoint checks on configs and tenant
  files, because a tenant made by `tenant create` holds only credentials. The path
  guards still apply: the store, the registry and every registered config must lie
  inside the sandbox.
- **The TypeScript endpoint trap (0.4, D93).** TypeScript `runtime topics create`
  (and, later, `apply` and `revert`) ignore the tenant's `mgmtEndpoint` and read
  `PANW_MGMT_ENDPOINT`, else production. For every TypeScript run whose first
  positional argument is `runtime` (`is_runtime_command`), the runner points each of
  `TS_ENDPOINT_VARIABLES` that the test did not set at the fixture server's
  `/ts-env-trap`, which answers `404 {}` and is recorded as `Route::TsEnvTrap`. Such a
  run that removes one of them (`without_env`) is refused before it starts
  (`removing <name> would fall back to production`), since an unset variable is the
  SDK's production default. A
  parity test that wants TypeScript to succeed sets
  `PANW_MGMT_ENDPOINT=server.ts_env_mgmt_url()` (`<base>/ts-env/aisec`, a second base
  that shares the fixture state; `RecordedRequest::base` tells the two apart).

### 5.3 Sandbox write check

Before each run the runner snapshots every file in the sandbox outside `tmp/`: the
registry, the configs, the Rust store (`store/`), `home/`, `cwd/` and sibling `*.lock`
files. After the run, a
change in bytes, mode or mtime, or a file that appeared or disappeared, fails the test.
This is how the read-only tenant invariant is tested, and how a failing write command is
shown to have created nothing (D87). The write tests grant airs-cli-rs `store/` and the
TypeScript CLI `state/` (where it keeps its registry and managed configs) with
`allow_writes_under`, and then assert the expected store files themselves. A test that
edits files itself
mid-run opts out with `RunOptions::allow_tenant_writes`. A narrower opt-out,
`RunOptions::allow_writes_under(<sandbox-relative path>)`, admits changes under one path
(and the directories created on the way to it) and nothing else: the `--debug` tests use
it for the default log directory (`home/.local/state/airs-cli`) and for TypeScript runs,
which write their log into `cwd`.

### 5.4 Both CLIs from the same fixture data (removed)

Removed on 2026-11-08 with the parity tests.

### 5.5 Prompts and the store lock

- **Pty runner** (`airs_test_harness::spawn_pty`, `crates/airs-test-harness/src/pty.rs`):
  the child gets the slave side of a new pseudo-terminal as stdin and stderr and a pipe
  as stdout (`spawn_pty_with(…, false)` keeps stderr piped, so the prompt gate fails).
  `expect` waits for text in the transcript (a `\n` matches the terminal's `\r\n`) and
  `send` types into it; `\x04` is Ctrl-D. The child is not the terminal's session
  leader (that needs `pre_exec`, which is `unsafe`), so the terminal sends it no signal:
  Ctrl-C is `PtySession::signal("INT")` for airs-cli-rs. The TypeScript CLI reads the
  terminal raw, so for it Ctrl-C is the byte `\x03`. `PtyRun::echo_restored` reads the
  terminal mode after the run. The transcript gets the leak check; every other runner
  invariant applies unchanged.
- **Lock holder** (`airs_test_harness::lock_holder`): a test binary runs itself as a
  process that holds `store/config.lock` with `flock`, as the CLI does, and writes the
  holder record. The `#[ignore]`d test `tests/tenant_write.rs::locking::lock_holder_helper`
  is the helper's body; `lock_holder::spawn` starts it and returns once the lock is held.
  `lock_holder::spawn_file` holds any lock file instead (the M5 job lock `<S>/<job>.lock`;
  helpers `tests/bulk_scan.rs::resume::lock_holder_helper` and
  `tests/parity_bulk_scan.rs::resume_divergence::lock_holder_helper`).
  It releases after a given time or when killed (the kernel releases a dead process's
  `flock`; its record stays in the file).

### 5.6 Leak check

stdout and stderr of every run are searched for the fixture secrets in each form a
leak could take: every secret-named config value raw, JS-trimmed, URL-encoded and as
unpadded base64; the HTTP Basic client pair; every token the fixture issued (through
the `mock-token-` prefix and its base64 form `bW9jay10b2tl`); and secrets a test adds
with `RunOptions::secret`, in the same forms. A match fails the test.
`RunOptions::allow_secret_output` turns the check off for one run.

The same forms are searched in every `--debug` log file the run left: under the child's
`XDG_STATE_HOME/airs-cli/debug/`, under `home/.local/state/airs-cli/debug/` and in the
working directory (the TypeScript log). `airs_test_harness::debug_log::assert_logs_clean`,
which every test in `tests/debug.rs` and `tests/parity_debug.rs` calls, also searches for
every `x-payload-hash`, `authorization` and `x-pan-token` value and every URL query the
fixture server received, for test-supplied text such as the scan prompt, and for any
`"body"` key.

The fixture server can issue a new token per request (`set_rotating_tokens`) and echo
the received bearer (`Planned::echoing_bearer`), so the redaction of refreshed tokens
is tested too.

For every airs-cli-rs `redteam` run the check also searches for the red team fixture
credentials (`fixtures::redteam::markers()`, `FAKE-TARGET-SECRET-01` … `-40`, in the same
forms): the seeded targets, adapters and error logs carry them in every credential path, so
a red team test cannot pass while a credential reaches stdout, stderr or a debug log (D140).
TypeScript runs print them (D140), so the parity tests turn the check off for the TypeScript
side only.

### 5.7 M4 write fixtures

The fixture server answers the management writes statefully (DESIGN §10 M4):
`POST /v1/mgmt/topic|profile` creates a record with a fresh id (`fixtures::uuid(901)`,
`902`, … in creation order) at revision 1; `PUT …/uuid/{id}` replaces the body's fields
and bumps the revision (`set_put_appends_new_record(true)` appends a new record with a
new id instead, as the service does for profiles); `DELETE /v1/mgmt/topic/{id}` answers
409 with the conflict payload while a profile references the topic; the `/force`
variants need a non-blank `updated_by` and strip the topic's refs. `server.topics(tsg)`
and `server.profiles(tsg)` read the state back. `push_once` and `set_default` still
override any route (`Route::TopicCreate`, `Route::ProfileForceDelete`, …); a 5xx on a
write should carry `with_retry_after(0)` so the TypeScript retries are immediate.
Parity tests compare the TypeScript outcome lines on stdout with the Rust stderr
(`split_outcome_lines`), request bodies as JSON and the resulting state. Confirmations
run on the pty: airs-cli-rs with stdin and stderr on it (`spawn_pty`), TypeScript with
stdout on it as well (`spawn_pty_all`), answering with `n\r`.

`topics apply` and `revert` were family-B commands on the TypeScript side before 7.3.0, so
their parity scenarios set `PANW_MGMT_ENDPOINT` to the `/ts-env` base (`family_b` in
`tests/parity_runtime_topics.rs`); from 7.3.0 TypeScript uses the tenant's endpoint and
ignores it (D93 narrowed). A change between `apply`'s profile read and its PUT is
simulated with a one-shot `Route::Profiles` page that differs from the stored record.
`topics eval` fixtures answer each prompt through `add_scan_verdict(marker, …)` with
`prompt_detected.topic_violation` set; concurrency is tested with every scan hanging
(`set_default(Route::ScanSync, Planned::hang())`), and the rate only with the lower bound
on the run time that the limiter guarantees. `--prompts` and `--output-file` are path
flags of the runner's guard, so their files live under `tmp/`.

`profiles cleanup` tests (`tests/runtime_cleanup.rs`, `tests/parity_runtime_cleanup.rs`)
seed several revisions of one name (the older ones listed first) and fail single
deletions with `push_once(Route::ProfileDelete, …)`: a 409 for a revision a policy
references, a 503 with `with_retry_after(0)` for an unknown outcome, a `Planned::hang()`
for an interrupted one. TypeScript cleanups always pass `--updated-by`, except the one
D109 test, which gives its `git config user.email` a sandboxed configuration
(`GIT_CONFIG_NOSYSTEM=1`, `GIT_CONFIG_GLOBAL=<sandbox>/tmp/gitconfig`,
`GIT_CEILING_DIRECTORIES=<sandbox>`). TypeScript's listing loop never ends on
`next_offset: 0` (D108), so the D108 test answers its first two pages explicitly; the
Rust walk is also tested with the server's `set_zero_next_offset_on_last_page(true)`
and with a repeated cursor.

Directional profile tests (8.0, DESIGN §13.17) seed TypeScript 7.3.0's own fixture,
`fixtures::runtime::directional_profile()` (`crates/airs-test-harness/data/directional-security-profile.json`,
copied verbatim from the tag): one `per_content_type` entry with all four directions,
detector severities and a `dlp_tenant_id`. A POST answer echoes the body's
`dlp_tenant_id`, as the service and the TypeScript spec's server do.
`tests/parity_runtime_directional.rs` runs every scenario of 7.3.0's
`tests/integration/directional-security-profiles.spec.ts` on both CLIs (its own `exec`
seeds the profile, the `Restricted` topic and `TMP/profile.json`); legacy and multi-entry
variants are built from the same fixture. The report, transfer and write parity files
each add a directional scenario.

### 5.8 `--debug` tests

`--debug` runs set `XDG_STATE_HOME=<sandbox>/tmp/xdg` (`debug_log::xdg_state_home`), so
the log lands under `tmp/`, which the write check ignores; `PRISMA_AIRS_TENANTS_PATH`
still selects the registry. `debug_log::only_log` and `debug_log::records` read the log
back (each line must be a whole JSON record). A real write failure is produced by running
the CLI through a `/bin/sh` wrapper that does `trap '' XFSZ; ulimit -f 0` before `exec`:
the empty log is created, every write to it fails with `EFBIG`, and stdout and stderr
(pipes) are unaffected (`tests/debug.rs::files::write_failure_keeps_the_outcome`).

### 5.9 Bulk-scan tests (M5)

`tests/bulk_scan.rs` and `tests/parity_bulk_scan.rs` drive the fixture's async routes
(`Route::ScanAsync`, `ScanResults`, `ScanReports`): receipts come from a per-server
counter (`fixtures::async_scan_id(n)`, `async_report_id(n)`), so both CLIs get the same
ids on fresh servers; `add_async_rule(marker, AsyncRule::…)` scripts a prompt's verdict,
failure or `pending` rounds; `push_once` overrides one request (a status with
`with_retry_after(0)`, `Planned::builtin()` to keep the built-in answer in a queue,
`Planned::held()` released by `release_held()`, `Planned::disconnect()`). Tests wait on
events (`wait_for_route`, the state file), never on timers. Every run sets
`XDG_STATE_HOME=<sandbox>/tmp/xdg` and an `--output-file` under `tmp/`; the TypeScript
fixture config sets `dataDir: <sandbox>/tmp/ts-data/runs`. A resume is a second run on
the same sandbox and server: `Fx::resume` (`tests/bulk_scan.rs`) and `Side::resume`
(`tests/parity_bulk_scan.rs`), with `sequence_since(mark)` for the requests of that run
only. A Rust state is never given to TypeScript; the D118 tests give a TypeScript state
to airs-cli-rs.

### 5.10 Red team tests (M6a)

`tests/redteam_reads.rs` and `tests/parity_redteam_reads.rs` drive the red team routes
(`Route::RedTeam(Rt::…)`) under the fixture tenant's `/redteam/data`, `/redteam/mgmt` and
`/redteam/broker` endpoints. `FixtureServer::seed_redteam()` loads the standard dataset
(`RedTeamData::seeded`: jobs, categories, both language answers, EULA, an instance, prompt
sets with and without version info, prompts, properties, four targets of different
connection and auth kinds, a target profile, error logs, adapters, channels and the
statistics); `redteam(|data| …)` edits it. Lists page with `skip`/`limit`;
`total_override` reports another `total_items` (an empty page before the total) and
`omit_totals` none. Downloads write under `tmp/` (or the granted `cwd/`); a race is
produced with `Planned::held()` on `Rt::TemplateDownload`. The fixture serialises with
sorted keys, so both SDKs print known keys in schema order and nested keys sorted.
Red team ids in the fixtures are fixed UUIDs from the `fixtures::redteam` helpers
(`job_id(n)`, `target_id(n)`, …); opaque ids (`job-1`, `t-1`) belong only in the `g25` test
modules. `src/redteam/contract_tests.rs` must change with any change to a red team
vocabulary or bound, or with an SDK pin whose vendor contract tightens one.

`tests/redteam_writes.rs` and `tests/parity_redteam_writes.rs` (step 2) drive the write
routes: every non-GET red team request is its own `Rt` (`Rt::is_write`) and changes the
dataset, so a later read sees it. A test proves the number of writes with
`server.rt_requests(Rt::…).len()`: one after a 5xx with `Retry-After: 0`, two after a 401
(`token, write, token, write`), one after a 403 (`attempts::every_write_is_one_attempt_d133`).
A scan's progress is scripted with `RedTeamData::scan_polls` (one patch per poll; once empty
the scan completes), so each extra round costs the real 5 s poll interval; the bound and the
other wait rules are unit tests with an injected interval
(`src/commands/redteam_scan.rs`). Interruption tests hold the write (`Planned::held()`),
signal the child, then release it, and check that nothing was re-sent. Confirmations run on
the pty (`spawn_pty`). The registry token is `REGISTRY_TOKEN`, passed as an extra secret
(`RunOptions::secret`) where it must not be printed. The parity tests compare the
TypeScript stdout after moving its outcome lines and hints to the Rust stderr (D145,
`ts_moved`), and the bodies with the client clock (`accepted_at`) masked.

`tests/redteam_reports.rs` and `tests/parity_redteam_reports.rs` (step 3) drive the report
routes (`Rt::StaticReport`, `StaticAttacks`, `DynamicReport`, `CustomReport`,
`CustomAttacks`) and the environment-report sources (`Rt::DashboardOverview`,
`ScanStatistics`, `TargetList`, `Quota`, `ScanList`, `ChannelStats`, `AdapterList`; the
`Rt::QuotaPost` trap must never be hit). The seed has a static report for job 1, a dynamic
one for job 2 and a custom one for job 3, which a test adds with
`data.scans.push(fixtures::redteam::custom_job())`; `attack_totals` changes the reported
attack total (the footnote). The environment report's parity test compares the HTML with
every `toISOString` time replaced by `<time>` and seeds old scan times, so the 24-hour window
does not depend on the clock. Backups are written under `tmp/` (or the granted `cwd/`) and
read back as JSON; the restore tests edit a backup file, and `Planned::builtin()` /
`Planned::held()` script one create at a time. The collector's TS unit cases run against a
scripted `Reader` in `src/report/tests.rs`.

### 5.11 DLP tests (M6b)

`tests/dlp_reads.rs` and `tests/parity_dlp_reads.rs` (step 1) drive the DLP read routes
(`Route::Dlp(Dl::List(Collection::…))`, `Dl::Get(…)`) under the fixture tenant's `/dlp`
endpoint. `FixtureServer::seed_dlp()` loads the standard dataset (`DlpData::seeded`: two
filtering profiles, five patterns of every status with one predefined and one
parent-managed, four profiles with a predefined `basic` one, two dictionaries);
`dlp(|data| …)` edits it. Lists are Spring pages (default size 20, `sort` applied to the
named field, keywords only with `keywords=true`); `styles` sets a collection's page
metadata (`PageStyle::Camel`, `Snake`, `NoLast`, `Bare`; the seeded dictionaries page in the
live snake_case style with `size_caps` 50), `ignore_page` repeats page 0 and `extra_pages`
reports an empty intermediate page. Custom records carry `FAKE-DLP-CONTENT-<nn>` markers in
their descriptions, regexes, proximity keywords, tag values and keywords: the runner fails an
airs-cli-rs DLP run (`is_dlp_command`) that prints one on stderr, on the pty or in a debug log,
and the tests assert where stdout may hold them (DESIGN §16.9). The parity tests compare the
JSON after applying TypeScript's camelCasing to the Rust tag and exclusion-list keys (D171).

`tests/dlp_writes.rs` and `tests/parity_dlp_writes.rs` (step 2) drive the write routes
(`Dl::Create`, `Dl::Replace`, `Dl::Patch`, `Dl::Delete` per `Collection`), which change the
dataset so a later read sees the change: created ids are `p-new-<n>`, `dp-new-<n>` and
`d-new-<n>`, every write bumps `version`, a pattern delete archives (`status: deleted`) and a
dictionary delete removes the record. `dictionary_replace_empty` makes the dictionary `PUT`
answer 204 without a body; `reserved_profile_names` makes a profile create answer 409. The
profile `DELETE` is a trap (501) that the write tests assert was never hit. `multipart(&request)`
parses an upload into its parts (names, filenames, content types, parsed JSON, length and
`sha256_hex`); the parity tests compare uploads by those and JSON bodies as JSON (D35), drop
airs-cli-rs's guard reads (D166) before comparing the request lists, and move TypeScript's
stdout success lines to the Rust stderr (D172). Confirmations run on the pty; `--force`
replaces them in the non-interactive runs (TypeScript has no `--force`, so its arguments omit
it). Interruption tests hold a write with `Planned::held()` (the service applies it on
arrival) and check that nothing is re-sent.

`tests/dlp_backup.rs` and `tests/parity_dlp_backup.rs` (step 3) drive `runtime dlp backup` and
`runtime dlp restore <file>` over the same routes. A restore test writes a backup with the Rust
CLI on one fixture server and restores it on another: `destination()` seeds only predefined
records under other ids and versions, with its own DLP `tenant_id` and
`DlpData::created_tenant_id` (the `tenant_id` the write routes give created records). Created
patterns get their `supported_confidence_levels` expanded (as the live service does, so the
restore reports `serverAdded`), and a dictionary upload whose file name ends `.csv` loses its
header row. The parity tests give TypeScript the same records in its own envelope (the two
backup formats differ, D175), compare backups by their summaries, counts and name sets, the
restore previews and results byte for byte or as JSON (the Rust-only `outcomeUnknown` and
`notAttempted` removed), and the creates by their JSON bodies and multipart parts; TypeScript's
`ℹ`/`✓` progress lines are dropped from its stdout before the comparison (D172). The recheck
test queues a builtin answer and then a crafted inventory page with `push_once`, so the second
sweep sees a change. Every test asserts that no `PUT`, `PATCH` or `DELETE` was sent and that no
`FAKE-DLP-CONTENT-<nn>` marker reached stdout.

### 5.12 Model Security tests (M6c)

`tests/modelsec_reads.rs` and `tests/parity_modelsec_reads.rs` (step 1) drive the
model-security read routes (`Route::ModelSecurity(Ms::GroupList)`, `Ms::ScanGet`, …) under
the fixture tenant's `/aims/mgmt` and `/aims/data` endpoints. `FixtureServer::seed_modelsec()`
loads the standard dataset (`ModelSecData::seeded`: the records of the TypeScript captures,
two groups, two rules, two rule instances of group `U(1)`, two scans with the evaluations,
violations and files of the first, two models, two versions, two label keys; ids
`fixtures::modelsec::uuid(n)`), which is parity-clean; `modelsec(|data| …)` edits it, and the
§12 variants are builders (`pending_scan`, `credential_uri_scan`, `custom_rule_instance`,
`uncounted_version`, a `file` without `formats`). Lists are skip/limit pages with
`pagination.total_items` (default limit 20, 50 on the model lists); `total_override`,
`omit_totals`, `ignore_skip` (repeated rows), `page_cap` and `count_capped` script the walk
cases. Sensitive values carry `FAKE-MS-SECRET-<nn>` markers: the runner fails an airs-cli-rs
model-security run (`is_model_security_command`) that prints one on stderr or in a debug log,
and the tests assert where stdout may hold them (DESIGN §17.15). The parity tests compare the
JSON after adding what TypeScript invents (`labels: []`, `formats: []`, `fieldValues: {}`,
D180) to the Rust side.

`tests/modelsec_writes.rs` and `tests/parity_modelsec_writes.rs` (step 2) add the write routes
(`Ms::GroupCreate`, `GroupUpdate`, `GroupDelete`, `RuleInstanceUpdate`, `ScanCreate`,
`LabelsAdd`, `LabelsSet`, `LabelsDelete`; `Route::is_write` holds for each), which change the
dataset: a create assigns `U(101 + n)` (groups, `PENDING`) or `U(451 + n)` (scans, `PENDING`,
no summary); a rule-instance `PUT` must name its path's group; the label routes merge, replace
or delete by the repeated `keys`. `ModelSecData::group_delete` (`GroupDeleteMode::Hard`,
`Tombstone`, `StillActive`) scripts what the re-read after a `DELETE` finds; planned answers on
`Ms::GroupGet` script a failed re-read. Configs are written into the sandbox's `tmp/` and passed
by absolute path. Every write test asserts the request count of its route (one attempt, D179),
and `Fx::ms` fails any run other than `scans create` that sends the billed `POST /v1/scans`.
The parity tests build each side's arguments in its own sandbox (TypeScript has no `--force`)
and compare the TypeScript stdout without its `✓` line to the Rust stdout, and that line to the
Rust stderr (D190).

`tests/modelsec_install.rs` and `tests/parity_modelsec_install.rs` (step 3) cover `pypi-auth`
and `install`. `Ms::PypiAuth` answers `https://oauth2accesstoken:FAKE-PYPI-SECRET-<tsg>@pypi.invalid/airs/simple/`
(`fixtures::modelsec::pypi_url`; `ModelSecData::pypi_url` replaces it with an http URL, no
userinfo, a query, …); the runner fails an airs model-security run that shows the
`FAKE-PYPI-SECRET-` marker on stdout, stderr or in a debug log. `airs_test_harness::fake_uv`
writes `<sandbox>/bin/uv` (`FakeUv`: the `--version` line, the exit codes, a sleep in `init` or
`add`, a stubborn sleep that only `SIGKILL` ends, echoing or persisting the credential, killing
itself) and, for the TypeScript fallback, `python3` and `pip`; each invocation records itself
under `tmp/uv-calls/<n>/` (argv, whether the credential variables and `UV_NO_CONFIG` are set and
the SHA-256 of the credential values, its pid and process group, the signals it trapped), read
back with `fake_uv::uv_calls`. Install runs set `PATH=<sandbox>/bin` only, so the real uv and
python never run. `--fetch-uv` runs use the debug-build hook (`AIRS_CLI_TEST_UV_ARCHIVE_URL` and
`AIRS_CLI_TEST_UV_ARCHIVE_SHA256`) pointed at `GET /uv-release/<file>`
(`FixtureServer::set_uv_archive`, `uv_archive_url`), whose archives `fake_uv::tar_gz` builds entry
by entry (`TarEntry`: files, links, pax paths, GNU long names, a bad checksum); the runner refuses
a `--fetch-uv` run without the hook. Every run's `XDG_DATA_HOME` is `<sandbox>/data`, so the
tools directory never reaches the real `~/.local/share/airs-cli` (grant it with
`allow_writes_under("data")`).

### 5.13 Runtime extras read, write and report tests (M6d steps 1–2)

`tests/runtime_extras_reads.rs` and `tests/parity_runtime_extras.rs` (step 1) drive `runtime api-keys list`,
`customer-apps list|get|consumption`, `deployment-profiles list` and the `scan-logs query` stub;
`tests/runtime_dashboard.rs` and `tests/parity_runtime_dashboard.rs` drive every `runtime dashboard` and
`runtime sessions` read. The routes are `Route::Runtime(Rx::…)` (`crates/airs-test-harness/src/server/runtime.rs`):
the management reads `Rx::ApiKeys`, `CustomerApps`, `CustomerApp`, `DeploymentProfiles` under the tenant's
`mgmtEndpoint`, and the dashboard reads `Rx::AppsOverview`, `Application`, `ApplicationViolations`,
`TopApplications`, `ViolationsTrend`, `AppsList`, `SessionsChart`, `SessionsOverview`, `Session`,
`SessionTransaction`, `ScanContent` under `mgmtDashboardEndpoint` (`/dashboard`; the fixture tenant points both at
the server, and the TypeScript side gets both explicitly). `Rx::ScanLogs` is a trap that no test may hit.
`FixtureServer::seed_runtime()` loads `RuntimeData::seeded` (the records of `fixtures::runtime`, ids
`fixtures::runtime::uuid(n)`: API keys 1–99, customer apps 101–199, three deployment profiles, the overview
applications and the sessions); `runtime(|data| …)` edits it and `runtime_data()` reads it back. The offset-cursor
lists take `repeat_rows` (every offset serves the first page), `repeat_cursor` (`next_offset` always `limit`),
`empty_first_page` and `zero_next_offset` (D12, D46, G28); the dashboard pages take `page_cap`, `total_override`,
`omit_totals` and `ignore_offset`; `customer_app_records`, `applications` and `breakdowns` replace single answers
by name or by `<appid>|<appname>`. A 401 or 403 pushed once with `push_once(Route::Runtime(rx), Planned::status(…))`
scripts the G7 re-read; `Planned::hang()` scripts a budget expiry.

`tests/runtime_extras_writes.rs` and `tests/parity_runtime_extras_writes.rs` (step 2) add the write routes
`Rx::ApiKeyCreate`, `ApiKeyRegenerate`, `ApiKeyDelete`, `CustomerAppUpdate` and `CustomerAppDelete`
(`Route::is_write` holds for each), which change the dataset: each key create or regenerate issues the next
`fixtures::runtime::uuid(900 + n)` with the value `FAKE-APIKEY-SECRET-<tsg>-<n>` (`keys_issued` counts them).
Four `RuntimeData` knobs shape the 2xx answers: `omit_key_value` (a key answer without `api_key`, a key the CLI cannot
save), `omit_auth_code` (a 2xx that fails the response schema after the key is saved, G26), `key_delete_answer` and
`app_delete_answer` (the delete answers, for the D208 acknowledgement). Every other answer is a generic `Planned`
override on the route: a status (401, 403, 5xx), an undecodable body (`Planned::raw`), `Planned::held()` (the write is
applied on arrival and the answer held until `release_held()`: a signal or a racing key file arrives while the request
is in flight) and `Planned::applied_then_dropped()` (applied, then the connection closed: an unknown outcome). The
one-attempt tests (D95) assert each write route's count with `rx_requests(rx)`. Key files are written into the
sandbox's `tmp/` and read back there (mode 0600, no `.airs-*.tmp` left unless the key was kept); runs that write
there need no grant. To send a signal after the key file is saved but before the outcome lines,
`RunOptions::stall_stderr()` stops the runner reading stderr, so the CLI blocks on its next stderr write once the
pipe is full; the test waits for the file, sends the signal with `Running::send_signal_recorded` (the run is spawned
with `RunOptions::signal_marker(<tmp path>)`; it returns once the CLI's signal thread has created that marker file,
DESIGN §2.5, a debug-build hook) and then calls `Running::release_stderr()`, so no fixed sleep decides whether the
signal was seen. Confirmations run on the pty (`spawn_pty`); `--force` replaces them in the non-interactive runs.

`tests/runtime_report.rs` and `tests/parity_runtime_report.rs` drive `runtime report` over the seven sources (the
five dashboard routes, `Rx::CustomerApps` and the management `Route::Profiles`); a source is made partial or
unavailable with `set_default` or `push_once` on its route, and a stalled source with `Planned::hang()`. Reports go to
the default name in the sandbox's `cwd/` (granted with `allow_writes_under("cwd")`), to an `--output-file` or to stdout
(`--output-file -`); the parity tests compare the reports byte for byte after normalising the timestamps (each CLI
reads its own clock). The collector and renderer cases of the
TypeScript `runtime.spec.ts` and `render.spec.ts` are unit tests (`src/report/runtime_tests.rs`).

For every airs-cli-rs runtime-extras run the runner's leak check (DESIGN §10 M6d) fails a run that shows a
`FAKE-APIKEY-SECRET-` marker anywhere (stdout, stderr, the pty, a debug log), a `FAKE-AUTHCODE-` marker anywhere but
stdout, or a `FAKE-PROMPT-CONTENT-`/`FAKE-RESPONSE-CONTENT-` marker anywhere but `scan-content --show-content`'s
stdout. The TypeScript side prints the markers, so its parity runs disable the check.

### 5.14 Runtime profile transfer tests (M6d step 3)

`tests/runtime_transfer.rs` and `tests/parity_runtime_transfer.rs` drive `runtime profiles
backup` and `runtime profiles restore <file>` over the management profile and topic routes
(`Route::Profiles`, `Topics`, `ProfileCreate`, `ProfileUpdate`, `TopicCreate`), whose rows are kept
per TSG (`FixtureServer::set_profiles`, `set_topics`), and the DLP data-profile list
(`seed_dlp`) for `--dlp-map`. A restore test backs up TSG 100 with the Rust CLI on one server and
restores the file on another whose tenant is TSG 200 (`--allow-other-tenant`), so the plan reads
an empty destination. `FixtureServer::set_profile_write_edit` edits every profile the write routes
store after answering (a server-added severity or `database-security: null`, or a changed policy)
to drive the read-back verification. Absent collections (D218) and crafted pages are
`push_once`/`set_default` answers on `Route::Profiles` and `Route::Topics`; a held, dropped or 5xx
`ProfileCreate` or `TopicCreate` scripts the one-attempt and interruption cases, and the pty helper
the confirmation and a concurrent change between the prompt and the recheck. The parity tests give
TypeScript the same records in its own envelope (`version`, `resourceType`, `source`; the formats
differ, D215) and compare backups by their summaries and records, restore previews and results
byte for byte or as JSON (the Rust-only `outcomeUnknown` and `notAttempted` removed), and the
writes by their JSON bodies. The planner cases of the TypeScript `runtime-profiles.spec.ts` are
unit tests over in-memory inventories (`src/runtime/transfer/plan_tests.rs`), and
`runtime-policy.spec.ts` runs against the `compareRuntimePolicies` port
(`src/runtime/transfer/policy.rs`).

### 5.15 Gateway read tests (M6e chunk 1)

`tests/gateway_reads.rs` and `tests/parity_gateway_reads.rs` drive `gateway workspaces list|get`,
`gateway scopes list|get` and every `gateway telemetry` read; the TypeScript side runs the same scenario as
`aigateway …` (D227). The routes are `Route::Gateway(Gw::…)` (`crates/airs-test-harness/src/server/gateway.rs`):
`Gw::DataWorkspaces` and `DataWorkspaceGet` under `/ai_gw/v2` (the fixture tenant's `aiGwDataEndpoint`, imported
as `gatewayDataEndpoint`), `AdminWorkspaces` and `AdminWorkspaceGet` under `/ai_gw/admin/v2`, `ScopeList` and
`ScopeGet` under `/iam`, and the telemetry routes `Chart` (`/logs/charts/<metric>`), `Group`, `Logs` and
`FilterBoundaries`. `Gw::DataWorkspaces` is also the TypeScript doctor's probe (it replaces the old
`Route::AiGwWorkspaces`; the unseeded dataset answers an empty list). Every gateway route needs the bearer of a
registered tenant and an `x-tsg-id` equal to its TSG (else 401 or 403). `FixtureServer::seed_gateway()` loads
`GatewayData::seeded` (`fixtures::gateway`: `U(1)` "Main A" visible on the data plane, `U(2)` "Production" on the
admin plane only, `U(3)` "Old" archived, three scopes, every chart body, the groups, three log rows and the filter
boundaries); `gateway(|data| …)` edits it and `gateway_data()` reads it back. Knobs: `list_total` and `has_more` on
the workspace lists, `scope_count`, `logs_total` and `logs_quota`; `data_plane` decides which workspaces the data
plane lists and `details` replaces a workspace's detail. Failures are `Planned` answers on the route (`set_default`
for a 403 the SDK re-reads once, `Planned::hang()` for a signal). Telemetry parity pins `--start`/`--end`; a test
that keeps the rolling window compares the two instants' span and the rest of the request line.

TypeScript gateway runs get `TZ=UTC` from the runner unless the test sets `TZ` (G41; the D242 test sets
`Asia/Kolkata` on both sides). For every airs gateway run the leak check fails a run that shows a
`FAKE-GW-SECRET-` marker anywhere (`fixtures::gateway::markers()`); the TypeScript side may print them (D238), so its
parity runs disable the check.

### 5.16 Gateway inventory read tests (M6e chunk 2)

`tests/gateway_inventory.rs` and `tests/parity_gateway_inventory.rs` drive every inventory and admin-guardrail
read (`api-keys`, `audit-logs`, `configs`, `deployments`, `guardrails`, `integrations`, `mcp integrations`,
`organisations`, `plugins`, `providers`, `admin-guardrails`). Their routes are the 32 `Gw::INVENTORY` routes
(`Gw::ServiceKeys` … `Gw::AdminMcpMappings`) over `GatewayData::inventory`, one body per route seeded by
`GatewayData::seeded_inventory` from `fixtures::gateway` (`service_keys()`, `audit_logs()`, `config_detail()`, …;
each valid for both SDKs' response schemas). A single-resource route answers only for its fixture id
(`fixtures::gateway::SERVICE_KEY`, `CONFIG`, …; 404 for another), the organisation routes only for the bearer's TSG
(403 for another), the workspace-scoped lists only for `workspace_id=U(1)` (an empty list for another). Tests edit a
body with `gateway(|data| { data.inventory.insert(Gw::…, …) })`. Every credential the bodies carry is a
`FAKE-GW-SECRET-<n>` marker (10–29 in the datasets, 30–69 in tests; see the fixture module); the runner fails an
airs-cli-rs run that shows one, so every read in every format proves the withholding policy. The parity tests compare
the credential-free reads byte for byte and the secret-bearing ones outside the withheld members (TypeScript's values
there masked in memory). The guardrail catalog is sent in a non-alphabetical member order
(`fixtures::gateway::service_order`), as the service sends it, so its table and pretty cells are compared as JSON
(D35); a `serde_json::Value` body is otherwise sent with sorted members, which hides a key-order difference.
The policy's secret paths come from the SDK's `AI_GATEWAY_SECRET_FIELDS`; `crates/airs-runtime-core/src/withhold.rs::tests::
every_sdk_secret_path_is_withheld` walks that table and fails when an operation (after an SDK bump) has no policy
and is not in the commented audit-only list, or when any request or response path is shown by its policy or by a
revealed audit field. `every_credential_member_is_withheld_everywhere` runs each credential member of rule 3 through
every policy and the revealed audit rule (at the root, at depth, inside a JSON text and as a stringified JSON value)
and asserts it withheld and registered; `tests/gateway_inventory.rs::withholding::config_texts_withhold_service_accounts_at_any_depth`
proves a stringified service account inside a config text withheld through `configs get` and `versions` in every format.
Rule 3f fails closed and rule 4 reads every URL inside a text (choices 167–169):
`d238_a_json_text_serde_json_cannot_read_is_withheld_whole` runs a lone-surrogate, an out-of-range-number, a
depth-129 and a truncated JSON text through a document, a header map, a credential name and a hidden and revealed
audit field; `d251_a_password_holding_a_delimiter_is_withheld` and `d251_urls_inside_a_text_are_withheld` pin the
URL parser (`values::urls_in` of the rules module, shared with the live check); each also asserts the shared checker
refuses the received text,
and `tests/gateway_inventory.rs::withholding::d238_d251_unreadable_config_texts_and_urls_in_prose_are_withheld`
proves both through `configs get` in every format.

### 5.17 Gateway write tests (M6e chunk 3)

`tests/gateway_writes.rs` and `tests/parity_gateway_writes.rs` drive every gateway write. Their routes are the 40
`Gw::WRITES` routes (`Gw::WorkspaceCreate` … `Gw::McpUpsert`, `crates/airs-test-harness/src/server/gateway_writes.rs`).
The workspace and scope writes change `GatewayData` (a created workspace `U(4 + n)` with slug `ws-<stem>-<n>` and a
read-back detail; the IAM scopes created, replaced and deleted; `scope_put_lost` drops one scope PUT to model a
concurrent writer); every other write answers `GatewayData::write_answers` (`fixtures::gateway::write_answers`, valid
for both SDKs) for its fixture id only, the deployment and integration deletes only with the bearer's TSG as
`organisation_id`, the auth-settings update only for the bearer's TSG. The one-time credentials of the key creates,
the rotate, the deployment create and a rotating deployment update are the markers 30–36: the runner fails an
airs-cli-rs run that shows one on a stream or in the debug log, so only the `--secret-output` file may hold them.
Failures are planned per test with `push_once`/`set_default` (401 once, 403, 400, 502/503 with `Retry-After: 0`, a
raw unreadable 2xx, held, dropped, applied then dropped) and every write test counts the requests of its route
(`gw_requests`) to prove one attempt. `--secret-output` paths are under the sandbox's `tmp/` (the flag is in
`PATH_FLAGS`); the confirmations run on the pty; the interruption tests hold the write and signal the process, and
the signal after a saved credential waits for the signal marker as in §5.13. The one-attempt rules are pinned on the
typed writes (`ConfigCreate`) and on the G36 raw credential writes (`KeyCreate`, `KeyRotate`, a rotating
`DeploymentUpdate`: the route order `token, write, token, write` after a 401, a 403 and an applied-then-dropped write
with no file left); the 401 replay also on both CLIs. Workspace references over several lists are tested with a
data-plane workspace named after an admin-only workspace's slug (choice 127) and a row with an empty slug. The
parity tests pass `--force` and the organisation flags TypeScript requires, compare the request lines and bodies as
JSON, the pretty and json answers, and TypeScript's stdout with its `✓`/`⚠` lines removed (D230).


### 5.18 Gateway inference, report and doctor tests (M6e chunk 4)

`tests/gateway_inference.rs` and `tests/parity_gateway_inference.rs` drive `gateway inference` against the fixture's
runtime routes (`Gw::InferenceChat`, `Gw::InferenceResponses`, `Gw::InferenceEmbeddings` under `/ai_gw/inference`,
`crates/airs-test-harness/src/server/inference.rs`): each checks `x-portkey-api-key` against
`fixtures::gateway::INFERENCE_KEY` and refuses any `Authorization` header (401), answers `GatewayData::inference` (JSON)
or, for a `"stream": true` body, `GatewayData::inference_streams` (SSE built by `fixtures::gateway::sse`, with a
keepalive comment). The tests put the key and both default models in the tenant (`aiGwInferenceApiKey`,
`aiGwInferenceModel`, `aiGwEmbeddingModel`, provisioned into the store as the `gateway*` keys), so the runner's leak
check forbids the key on every stream. A secret split across two deltas, control characters in model text, a failed
lifecycle, a broken stream, provider errors (one attempt), a timeout, the debug log (the key header masked, no prompt
or output) and SIGINT/SIGTERM on a held request are each a test; the D240 trap passes `PANW_AI_GW_INFERENCE_*` through
`RunOptions::env` to a tenant without a key (TypeScript sends, airs-cli-rs refuses). Model calls are billed: the live
tier never reaches these routes.

`tests/gateway_report.rs` and `tests/parity_gateway_report.rs` run `gateway report` over the seeded dataset with a
pinned window (`--start 2026-10-01T00:00:00Z --end 2026-10-02T00:00:00Z`, around the fixture's log rows); the parity
file compares the HTML byte for byte with the collection times normalised, stderr and the 25 reads (TypeScript with
`TZ=UTC`). The collector's TS spec cases are unit tests in `src/report/gateway_tests.rs` over a scripted `Reader`.
The doctor `Gateway API` cases are `tests/doctor.rs::gateway_api` and the `d227`/`d250` tests of
`tests/parity_doctor.rs`; the doctor's other tests now expect eight checks.

### 5.19 The gateway withholding rules (adding a name or a sub-rule)

The rules of DESIGN §19.3 rules 3 and 4 live in the CLI's module `crates/airs-runtime-core/src/withhold_rules/` (choices 172, 185;
DESIGN §11 names where a change starts and the tests that pin each file, choice 188),
`std` and `serde_json` only: `names.rs` (the normalisation, the `NAME_RULES` table, the action table, headers, pairs,
registration and its one filter `registrable` with the placeholder and mode words, choices 183, 188), `values.rs` (the string tests, the URL parser, the
scheme, header and key=value rules, `text_withheld`, the four text rules in order as the CLI applies them, and
`model_output_withheld`, the URL and scheme rules of model output), `view.rs` (the read-only `Json` view both
traversals read shapes through, implemented in `withhold.rs` for the CLI's value), `check.rs` (the read-only checker:
`check::violations` names the path of every value that shows what the rules withhold, `check::withheld` is "none";
its `Document` trait, a `Json` that parses a text, is implemented there for `serde_json::Value` and in the CLI's tests
for the CLI's value; `#[cfg(test)]` in the CLI, so the binary never carries it, choice 187) and
`corpus.rs` (the pinned and the generated documents; `REAL_WORLD_NAMES`, a fixed list of credential names written
without the rules; and `REAL_WORLD_TEXTS`, strings that hold a credential in practice and prose the text rules must
leave, each with its printed form). The test harness compiles the same files by `#[path]`
(`crates/airs-test-harness/src/withhold.rs`: the module root, and `check.rs` and `corpus.rs` for the live tier and
the workspace's other tests as `airs_test_harness::withhold::{check, corpus}`), so inside them a sibling is
`super::`, never `crate::`; a helper only the tests or the harness call is `#[cfg(test)]` or carries its own narrow
`allow` (no blanket `dead_code` allow on the module, choice 187);
`withhold::tests::corpus::the_rules_compile_outside_the_cli` holds that. `corpus` is test data: `#[cfg(test)]` in the
CLI, never in its binary (choice 182). `crates/airs-runtime-core/src/withhold.rs` is the CLI's rewriting traversal; it asks the rules
module for every decision. Its tests are
`crates/airs-runtime-core/src/withhold/tests/`: `name_rule.rs` (rule 3 by name, registration, the DESIGN table), `headers.rs` (3c),
`schemas.rs` (3e), `paths.rs` (rules 1 and 2), `audit.rs` (rule 5), `texts.rs` (3f, private keys, the URL, scheme
and key=value rules, model output) and `corpus.rs` (the corpus drivers and the traversal-equality check), with the
shared helpers in `mod.rs`.

- **A name.** Add it to the list of the row it belongs to in `NAME_RULES` (a printed exception to `PRINTED_NAMES`, a
  credential word to `CREDENTIAL_FRAGMENTS`, …), then regenerate the "Rule 3 by name" table of DESIGN §19.3 from
  `names::rules_table()`: `cargo test --lib the_design_table_is_the_name_rules` prints the table it expects when the
  two differ. Add a §19.20 choice and, where it changes what TypeScript prints, a D238 COMPATIBILITY note. The
  generated corpus picks the word up in every form and container shape by itself (`corpus::credential_names` reads
  the rows); a printed exception needs a case in `d238_more_credential_name_shapes_are_withheld` or the test of its
  choice. A credential name found in the wild goes into `corpus::REAL_WORLD_NAMES` first (it is not derived from the
  rules, so `every_real_world_credential_name_is_withheld_by_name` and the planted-credential tests fail until a row
  claims it); a name deliberately printed goes into the exclusions noted beside that list instead.
- **A text shape** (a credential inside a string under a neutral name: a command line, a connection string, an
  error text): add a `RealWorldText` to `corpus::REAL_WORLD_TEXTS` with its printed form, the credentials it must
  register and the words that must stay printed elsewhere (`kept`, the limit in the other direction).
  `withhold::tests::corpus::d238_real_world_texts_print_as_pinned` runs each through the CLI and `check::violations`,
  the harness's `the_real_world_texts_are_checked_as_the_cli_prints_them` through the `serde_json` checker and
  `values::text_withheld`, and `withhold_rules::corpus::tests::every_real_world_text_registers_as_pinned` through
  `text_withheld` alone (what it registers, and that no `kept` word is carried). A value that only says a
  credential is absent or masked (`auth=none`, `Pwd=****`), or a short word of letters and digits (`auth=oauth2`),
  belongs in `kept`: the registration rule must leave it out (choices 183, 186).
- **A kind of name** (a new matching rule): add a row (a `Pattern` and a `Class`) and give `corpus::credential_names`
  the forms it claims; `every_generated_name_is_a_credential_name_of_its_row` fails when a generated form does not
  classify as its row. A new `Class` (choice 187's `Fingerprint`) also needs its column of the action table
  (`names::action`, `names::header_action`), its place in the class order (which of two schema names wins), its arm in
  `withhold.rs::path_leaf` and `view::text_free`, and its label in `names::rules_table`; a value shape it reads
  (`Shape::CredentialLike`) comes from `view::shape`, which both traversals call.
- **A value type** (a number, the empty string): the action table decides it per class (choice 187: a number and the
  empty string are withheld under every withholding class but a count's number and a generic header's, `null` and
  booleans printed). `corpus::planted` plants an eight-digit number under every credential name and member in every
  container shape and echoed, so a class that starts printing numbers fails
  `d238_a_planted_credential_never_prints_in_any_format`.
- **A sub-rule** (a new structure, such as the `{name, value}` pair of choice 170): change both traversals, the CLI's
  (`withhold.rs`) and the checker (`check.rs`), through the same rules function.
  `the_checker_names_every_value_the_cli_changes` holds them equal (choice 179): for every pinned and generated
  document as received, the paths the CLI's traversal changes must be the paths `check::violations` names, so a
  sub-rule changed on one side only fails there with the paths that differ. Add one `(received, printed)` case
  to `corpus::PINNED_DOCUMENTS` (the CLI's `the_live_tier_accepts_what_the_cli_prints` checks the printed form and
  that the checker refuses the received one; the harness's `the_pinned_documents_are_checked_as_the_cli_prints_them`
  runs it on `serde_json::Value`) and, when it is a new container, a shape to `corpus::containers`, so
  `d238_a_planted_credential_never_prints_in_any_format` (every name shape × every container × json, pretty and
  table, and a revealed audit body) and the harness's `every_planted_credential_is_refused` cover it. Add a `d238_`
  test in the matching file of `crates/airs-runtime-core/src/withhold/tests/` and, for a command-level effect, one in
  `tests/gateway_inventory.rs`.
- **Scale.** The `SecretSet` is read once per printed string, so a rule that registers per row must stay linear:
  `withhold::tests::audit::d239_a_large_audit_log_is_withheld_in_linear_work` reads 4000 audit records and bounds
  structure, not time (no unrevealed field text registered, at most four forms per record, at most two matcher steps
  per printed byte, read from `SecretSet::form_count` and `SecretSet::steps`, both `#[cfg(test)]`: the step count is
  `secrets::Steps`, zero-sized outside a test build, and a `const` assertion in `crates/airs-runtime-core/src/secrets.rs` fails the binary's
  build if it ever carries one), so it is deterministic in CI; the matcher itself is checked against a `contains`
  per form (`crates/airs-runtime-core/src/secrets.rs::tests::the_matcher_agrees_with_contains_per_form`).
  A wall-clock measurement belongs in the choice that motivates a change (choice 187), never in a test.
- **The live check** needs no change: `live::gateway::withheld` is the same checker, compiled into the harness
  from `check.rs`. A new file of the rules module needs its `mod` line in `withhold_rules/mod.rs` (the harness
  compiles the module root; a test-only file is `#[cfg(test)]` there and included again by the harness, as `check.rs`
  and `corpus.rs` are) and the file list of `the_rules_compile_outside_the_cli`.

### 5.20 Supplychain tests (M6f)

`tests/supplychain_reads.rs` and `tests/parity_supplychain_reads.rs` drive `supplychain scans list|ls`,
`scans vulnerabilities`, `stats` and `rules list|ls`; the TypeScript side runs the same scenario as `agentguard …`
(D308). The routes are `Route::Supplychain(Sc::…)` (`crates/airs-test-harness/src/server/supplychain.rs`): `Scans`,
`Stats` and `Vulnerabilities` under `/agentguard/data/v1` (the fixture tenant's `agentGuardDataEndpoint`, mapped to
`supplychainDataEndpoint` in the Rust store), `Rules` under `/agentguard/mgmt/v1`; only `GET` is classified, and every
route needs the bearer of a registered tenant and an `x-tsg-id` equal to its TSG (else 401 or 403).
`FixtureServer::seed_supplychain()` loads `SupplychainData::seeded` (`fixtures::supplychain`: 12 scans `U(101)`–`U(112)`,
three findings of `U(101)`, the statistics, 25 rules `U(201)`–`U(225)`); `supplychain(|data| …)` edits it and
`sc_requests(Sc::…)` reads the requests. The bodies keep the service's member order: they are written from
`fixtures::supplychain::Wire`, an ordered JSON value, never from a `serde_json::Value` (which sorts), and carry nested
objects, `null` members and passthrough members out of alphabetical order (`zz_added` before `aa_added`; DESIGN
§20.13). Knobs: `scan_total` (a reported total other than the count), `finding_total` (D319), `oversized_findings` (a
33 MiB answer, D320), `pagination_passthrough`, and `without_passthrough()` for the parity scenarios that compare whole
records byte for byte (D35 orders passthrough members differently, so `d35_passthrough_members_are_sorted` compares
them semantically). Planned pages for the page checks come from `fixtures::supplychain::scan_list_body` and
`rule_list_body`.

**Scan rows with `null`s (G71, D324, D325).** The live service sends `null` where both SDK contracts require a scan
member (`"fingerprint": null` observed), so the scan list is read raw through a tolerant projection (DESIGN §20.16).
`fixtures::supplychain::null_scans()` (113–115, `NULL_SCANS`) are real-shape rows with those `null`s in the service's
member order: 113 the observed row, 114 every member but `uuid` `null`, 115 partial nulls and a `null` count;
`recent_null_scans()` gives the report's window. `deep_scans(depth)` nests the first scan's `eval_details` so the page's
deepest value sits at `depth` (`json_depth`; 64 passes, 65 is G66's `Limit`, DESIGN §20.16 choice 77). Seed them with `supplychain(|data| data.scans = fx::null_scans())`; the
seeded dataset itself stays schema-valid, so every existing parity scenario still compares pages both SDKs accept. A
case that must still fail the contract removes a scan's `uuid` or gives a read member another type (the contract line,
D325); a schema failure of a typed read uses another route (a statistics answer without a metric). The byte identity
of the tolerant projection with the typed model is a unit test over the seeded pages
(`src/supplychain/scans_tests.rs::accepted_answers_project_as_the_typed_model_did`): when the projection changes, that
test must keep passing, and a new projected member needs a fixture row carrying it as `null`. The over-withholding
rule of the 0.10 lesson applies to the nulls too: `tests/supplychain_reads.rs::g71` asserts the ordinary row and every
non-null member print unchanged beside the null rows. The raw read must keep the typed call's transport rules on its
own path (`g71::the_raw_scan_read_keeps_the_typed_calls_rules`: a 302 on `Sc::Scans` not followed, the 32 MiB size line,
401 and 403 then 200 as token, GET, token, GET, an empty 2xx body as the request line), and its `--debug` name on every
page (`g71::no_scan_page_is_logged_as_a_raw_read`: the single page, the G63 re-read and each page of a walk are
`airs.agentguard.list_scans`, never `custom.read`). That last test guards `debug::named`'s pin constraint: the
task-local name reaches the transport only while the SDK awaits it in the caller's task, so run it first when moving
the scm-sdk pin. In the report a row is a parent when either `is_batch` is `true` or `child_scan_uuids` lists a child;
only a row neither decides is of unknown membership (`report::supplychain_tests::either_batch_signal_decides_a_parent`
covers the mixed cases).

For every airs supplychain run the leak check fails a run that shows the `PRIVATE-` marker anywhere; a run with
`--include-content` may show it on stdout only (`is_supplychain_command`). Ordinary data that only looks sensitive
(`fixtures::supplychain::ORDINARY_PATH`, `ORDINARY_CODE`, `ORDINARY_RULE_TEXT`) carries no marker, and the
over-withholding assertions check that it prints unchanged; a planted copy of the tenant's `mgmtClientSecret` is
`[withheld]`. Timeouts are not tested with a clock: the dropped connection (`Planned::disconnect()`) and the hang and
interrupt tests cover the transport paths.

**The report (chunk 2).** `tests/supplychain_report.rs` and `tests/parity_supplychain_report.rs` drive
`supplychain report` (TypeScript `agentguard report`); the collector's cases (TypeScript `unit/reports/agentguard.spec.ts`)
are unit tests in `src/report/supplychain_tests.rs` against a scripted `Reader` of aggregate-only rows. The report
filters scans on the run's own 30-day window, so its tests seed `fixtures::supplychain::recent_scans()` (the 12 scans
with their creation times an hour before the test; 109 stays in 2020, 110 loses its zone, 111 is today's date only,
112 is `garbage`; DESIGN §20.13); the parity scenarios that compare bytes use `recent_zoned_scans()` (no zone-less
row: TypeScript reads it in the host's zone, D322) and the same `--title` on both sides (D309), and normalise every
`YYYY-MM-DDTHH:MM:SS.mmmZ` (the run's times) and the `start_time`/`end_time` query values. `d322_report_times_need_a_zone`
runs TypeScript with `TZ=UTC`. The signal after the report is saved
(`interrupt::a_signal_after_the_report_is_saved_names_it`) uses `RunOptions::fill_stderr()` instead of
`stall_stderr()`: the runner starts the CLI on a stderr pipe it has already filled (the filler is not captured), so the
report's first stderr write, its `Report →` line after the publish, blocks until `Running::release_stderr()`; a merely
stalled stderr never fills from those short lines, and the run could exit before the signal (DESIGN §20.17 choice 90).
The doctor's management-credential line (D312) is `tests/doctor.rs::credentials` and
`tests/parity_doctor.rs::divergence::d312_the_doctor_names_supplychain_commands`.

### 5.21 The program names and the doctor's Executable check (1.0, 8.0, DESIGN §21)

Every test runs `env!("CARGO_BIN_EXE_airs")`. The other binaries, `CARGO_BIN_EXE_airs-cli`
(8.0) and `CARGO_BIN_EXE_airs-cli-rs`, run only in the tests that prove they are the same
program: `tests/shell.rs::compat` (the same exit code, stdout and stderr as `airs` for help,
version, parse errors, a usage error, an empty store and doctor without a tenant; a
symlink named `renamed-tool` still prints `Usage: airs …`),
`tests/parity_shell.rs::d326_the_program_is_airs_with_a_compatibility_name` and
`tests/parity_shell.rs::d330_airs_cli_is_a_name_of_the_rust_cli`.

The doctor's first check, `Executable` (D331), looks for `airs`, `airs-cli` and
`airs-cli-rs` on `PATH`. The runner therefore removes every inherited `PATH` entry that
holds one of them (`airs_test_harness::path_without_programs`), so an installed copy on
the developer's machine never changes a doctor test; `tests/doctor.rs` and
`tests/parity_doctor.rs` expect the `Executable` pass first. The tests of the check itself
(`tests/doctor.rs::executable`, `d331_the_executable_check_is_rust_only`) set `PATH`
explicitly, with stub programs and links under the sandbox or `CARGO_TARGET_TMPDIR`.
`tests/program_name.rs` reads files and runs help only, with no tenant and no request:
the source scan (every `airs-cli-rs` in `src/` is one of the four kept format names), the
docs scan (no current document, RELEASING.md included, invokes `airs-cli-rs`), the shim and `Cargo.toml` check, and a walk of every command's
`--help` (the usage line names `airs <path>`, every example starts `  $ airs `). The
harness labels (`Cli::label`, the live summaries' `airs-cli-rs/TypeScript`) name the
implementation and are unchanged.

### 5.22 Release packaging and the Homebrew formula (1.0, DESIGN §21.7–§21.9)

These suites run shell scripts, not the CLI, and never contact a host. Each script runs
with a cleared environment, `HOME` and `TMPDIR` inside a fresh directory under
`CARGO_TARGET_TMPDIR`, and a fixed `PATH`.

- `tests/homebrew_formula.rs` runs `scripts/homebrew-formula.sh` with a stub `curl` first
  on `PATH`. In the offline tests the stub records that it ran and exits 97, and every
  test checks it never ran: `--checksum` and `--metadata-file
  tests/fixtures/homebrew/crates-io-airs-cli-1.0.0.json` (a synthetic answer) both render
  exactly `tests/fixtures/homebrew/airs-cli-1.0.0.rb`, `--output -` writes stdout, an
  existing formula is replaced, and every refusal exits 2 (with the usage) or 1 and
  writes nothing (an answer file holding two JSON documents, or none, is `the answer is
  not JSON`). In `crates_io_answers_are_confirmed_by_the_downloaded_crate` the stub
  serves local files instead and records its arguments (`-q` first so no `.curlrc` is
  read, https only, the tool's user agent, no credential); a download that does not
  hash to the answer's checksum, a failed download (`cannot fetch <url>/download (curl
  exit 22)`) and a failed answer request are refused with nothing written. `the_template_and_the_fixture_are_the_designed_formula` checks that the
  fixture is the template rendered for 1.0.0 and runs `ruby -c` on it when ruby is on
  `PATH`. No test writes the script's default output, `target/homebrew/Formula/airs-cli.rb`,
  so a formula you rendered for the tap is never replaced by the fixture. When the
  template changes, re-render the fixture with
  `scripts/homebrew-formula.sh 1.0.0 --checksum <the fixture checksum> --output
  tests/fixtures/homebrew/airs-cli-1.0.0.rb` (the checksum is the `CHECKSUM` constant of
  the test).
- `tests/release_packaging.rs`: everywhere, `bash -n` and the executable bit of
  `scripts/release.sh`, `scripts/package-release.sh`, `scripts/homebrew-formula.sh` and
  `scripts/npm-package.sh`,
  `release.sh`'s argument refusals (they run before its first step, so nothing is read
  or contacted), its build, sign, package and naming lines read from the script (the
  asset names come from `SHA256SUMS`), and `package-release.sh`'s refusal of an empty
  argument (it precedes the bsdtar check). On macOS, the `packaging` module runs
  `scripts/package-release.sh` on stub binaries: the tarball layout (`airs`, the
  `airs-cli` and `airs-cli-rs` links, `README.md`, `LICENSE`, `NOTICE`, root-owned,
  0755/0644, equal to their sources), `SHA256SUMS`, the refusals (nothing written), the cleanup after a failed
  check, and `--verify-signature` with the real `codesign --verify --strict`: a copy of
  the Apple-signed `/usr/bin/true` passes and an unsigned stub fails (nothing is signed;
  the test notes a skip where `/usr/bin/codesign` cannot verify the copy). `release.sh --dry-run` itself needs `origin`, the
  Forgejo API and the keychain, so it is run by hand (RELEASING §2), never by a test.

### 5.23 The docs site's command reference

`tests/cli_reference.rs` walks `airs <path> --help` from the root down (each help's
`Commands:` block), with no tenant, a cleared environment and `PATH=/usr/bin:/bin`, so no
request is sent. It renders one CommonMark page per top-level command plus `index.md` (each
command's help in a fenced `text` block under a heading naming the command) and compares
them with the committed pages under `docs-site/docs/reference/commands/`. A stale, missing
or extra page fails the test, naming the files and `scripts/cli-reference.sh`. After
changing a command, a flag or a help text, run `scripts/cli-reference.sh` (it runs the test
with `AIRS_UPDATE_CLI_REFERENCE=1`, which rewrites the pages and removes extra ones) and
commit the result; `scripts/cli-reference.sh --check` runs the check alone. The docs site
build itself (broken links and anchors fail it) runs in the CI `docs-site` job (§10).

### 5.24 The npm channel (8.0, DESIGN §21.16)

`tests/npm_package.rs` needs node (`AIRS_TS_NODE`, else `node` on `PATH`; npm is taken
from node's directory) and never contacts a host other than loopback.

- `the_launcher_tests_pass` runs `node --test npm/test/launcher.test.js` (also runnable
  by hand): the platform table and the manifest's aliases, the refusals for unsupported
  platforms and a missing platform package, and, on a supported host, a copy of the
  launcher with a stub binary in its platform package: arguments, stdin, stdout, stderr
  and exit codes pass through, the binary runs under its own name and gets
  `AIRS_NPM_LAUNCHER`, a signal death is re-raised, and SIGTERM, SIGINT and SIGHUP sent
  to the launcher (no terminal) reach the binary.
- `npm_package_refuses_bad_input_and_writes_nothing` (everywhere): the argument
  refusals (exit 2 with the usage) and a missing dist, a missing `SHA256SUMS`, one
  listing other files and one that does not match (exit 1), with nothing written.
- On macOS (the input tarballs come from `scripts/package-release.sh`, which needs
  bsdtar), the `staging` module packages this build's `airs` and Linux stubs, runs
  `scripts/npm-package.sh` and checks the staged manifests, the binaries, every packed
  file list, `npm pack --dry-run` and `SHA256SUMS`; then
  `an_install_from_the_staged_tarballs_runs_both_names` runs
  `npm/test/install-check.mjs`, which serves the staged tarballs from a loopback
  registry and installs `@cdot65/prisma-airs-cli` with `npm install --global` into a
  temporary prefix (empty user and global npm configuration, private cache, no audit,
  fund or update check): only the darwin-arm64 platform package is installed,
  `airs --version` and `airs-cli --version` print the version, and doctor's `Executable`
  check passes through the npm links.

## 6. The live tier (removed)

The live comparison tier (both CLIs against a real tenant) was removed on 2026-11-08
with the parity tests.

## 7. Adding tests

### 7.1 A unit test

Put it in the `#[cfg(test)] mod tests` of the module it covers. Unit tests must not
start processes or open sockets.

### 7.2 A Rust-only executable test

Add it to the `tests/*.rs` file for the area (`tenant.rs`, `doctor.rs`,
`management.rs`, `scan.rs`, `shell.rs`). The usual shape:

```rust
use airs_test_harness::{Cli, FixtureServer, Route, RunOptions, Sandbox, fixtures, run};

fn rust() -> Cli {
    Cli::rust(env!("CARGO_BIN_EXE_airs"))
}

#[test]
fn profiles_list_prints_the_latest_revision() {
    let server = FixtureServer::start();
    let sandbox = Sandbox::new();
    sandbox.single_tenant(&server, "prod", "100"); // configs/prod.json + store/, active
    server.set_profiles("100", vec![fixtures::profile(1, "alpha", 1)]);

    let out = run(&rust(), &sandbox, &["runtime", "profiles", "list"], &RunOptions::new());
    out.assert_success();
    assert_eq!(out.json()[0]["profileName"], "alpha");
    assert_eq!(server.routes(), vec![Route::Token, Route::Profiles]);
}
```

Failures are injected with `FixtureServer::push_once` (the next matching request, from
either CLI) or `FixtureServer::set_default` (every request), using `Planned`
responses, delays and hangs. The crate documentation in
`crates/airs-test-harness/src/lib.rs` describes every piece.

### 7.3 A parity case

Add it to the matching `tests/parity_*.rs` (`parity_shell`, `parity_tenant`,
`parity_doctor`, `parity_management`, `parity_scan`, `parity_debug`). Get the reference with
`require_ts("<test name>")`, which returns `None` only when skipping is allowed:

```rust
let Some(ts) = airs_test_harness::require_ts("profiles_list_parity") else { return };
```

Run the same scenario on each CLI with a fresh sandbox or with the server's request log
cleared between runs, then compare exit code, stdout, stderr and
`normalize_all(&server.requests())`. A one-shot `push_once` response is consumed by the
next matching request from either CLI, so queue it once per CLI run, right before that
run. Each parity file has its own helpers for running both sides; reuse them.

A parity test asserts identical behaviour. If the two CLIs differ on purpose, it is a
divergence (§7.4), not a parity case with relaxed assertions.

### 7.4 A divergence

When `airs-cli-rs` should behave differently from `airs-cli`:

1. Add a row to DESIGN §12 with the next free number (`Dnn`), and update the DESIGN
   section that specifies the behaviour (AGENTS.md).
2. Add a test whose name contains a `dnn` token (usually a `dnn_` prefix, for example
   `d6_failed_scans_fail_closed`) to the matching `tests/parity_*.rs`, usually in its
   `divergence` module. It must assert what each side does, not only the Rust side.
3. Add the row to the divergence index in [COMPATIBILITY.md](COMPATIBILITY.md) §4,
   citing the test as a backticked
   `tests/parity_<area>.rs::<module>::<function>` reference. If no executable test is
   possible, write "no executable test" and the reason.
4. Run `cargo test --test docs_consistency`. It fails when a cited test does not exist
   in the named module, when a §12 row has no index row or the other way round, when
   COMPATIBILITY.md mentions a `Dnn` that §12 does not define, when an index row
   neither cites a test nor says "no executable test", and when a `dnn`-named test in a
   `tests/parity_*.rs` file is not cited.

Renaming or moving a cited test also needs a COMPATIBILITY.md update.

## 8. Behaviour not covered by an executable test

These are listed in DESIGN and in the COMPATIBILITY.md divergence index:

| Item | Coverage | Why there is no executable test |
|---|---|---|
| D31: the 60 s per-request and 300 s per-walk budgets | The 5 s doctor budget is tested on both sides; the constants are in `crates/airs-runtime-core/src/clients.rs`. | A test would have to hang a request for 60–300 s. |
| D36 and DESIGN §2.6: proxy use for https endpoints and macOS system proxies | The doctor's proxy suffix and the loopback proxy bypass are tested. | The fixture serves only plain-http loopback, so there is no https endpoint to proxy. |
| D40: SDK safety bounds on `--all` walks (100,000 pages, 512 MiB, 1,000,000 items) | Unit tests in `src/pagination.rs`. | Reaching a bound would take minutes and gigabytes per run, and the TypeScript CLI has no bound to compare against. |
| D58: colour decided per stream from whether each one is a terminal | The environment rules are tested on both sides, the per-stream decision by unit tests (`crates/airs-runtime-core/src/style.rs`, `src/output.rs`). | The runner pipes both streams. The pty runner (§5.5) puts stdin and stderr on a terminal for the prompt tests, but keeps `NO_COLOR=1`, so no test drives the terminal colour decision. |
| D47: server text carrying a secret in scan output | Tested on the Rust side. | A parity test is possible but does not exist; it would need `RunOptions::allow_secret_output` for the TypeScript run. |
| D48: a non-loopback `http` endpoint | Tested on the Rust side; the fragment case is parity tested. | The harness never lets the TypeScript CLI reach a non-loopback host, so its sending side cannot run. |
| Behaviour against the real service | The live tier (§6). | Needs a real tenant; see §6.11 for what has not been checked yet. |

## 9. Benchmarks (removed)

The benchmark harness (`examples/bench.rs`) compared both CLIs and was removed with the
parity tests on 2026-11-08. [BENCHMARKS.md](BENCHMARKS.md) keeps the last results.

## 10. Continuous integration

`.forgejo/workflows/ci.yml` runs on every push to `main` and to `ci/**` branches, on
pull requests, and on demand (`workflow_dispatch`). It never sets `AIRS_RS_LIVE`; the
live tier (§6) is only run by hand. Releases are cut separately
([RELEASING.md](RELEASING.md)).

| Job | Runner | What it runs |
|---|---|---|
| `linux` | `prisma-airs-cli` (Ubuntu 22.04 container, as root) | rustup stable with rustfmt and clippy; Node 24 (the npm launcher tests); `jq` (installed with `apt-get` when the image lacks it, for `tests/homebrew_formula.rs`); `cargo fmt --all --check`; `cargo clippy --workspace --all-targets --locked -- -D warnings`; `cargo +1.89 check --workspace --all-targets --locked` (the MSRV); `AIRS_REQUIRE_PARITY=1 cargo test --workspace --locked --no-fail-fast` |
| `musl` | `prisma-airs-cli` | `cargo zigbuild --release` for `x86_64-unknown-linux-musl` and `aarch64-unknown-linux-musl` (zig 0.17.0, checksum-verified; cargo-zigbuild 0.23.4); a smoke test of the native binary (`--version` of `airs` and of the compatibility name, then `tenant list --output json` with `AIRS_CLI_CONFIG_DIR` pointing at an empty directory, which must print `[]`); both `airs` binaries uploaded as artifacts `airs-cli-<target>` with an `airs.sha256` file |
| `docs-site` | `prisma-airs-cli` | Node 24 and pnpm 10.6.5 (from `docs-site/package.json`'s `packageManager`); `pnpm -C docs-site install --frozen-lockfile`; `pnpm -C docs-site build`, where `onBrokenLinks` and `onBrokenAnchors` are `throw`, so a broken link or anchor fails the job. The Forgejo `docs-release` workflow requires it (with `linux` and `musl`) to pass on `main` before it tags `airs-docs-<sha>`, which the GitHub mirror's `deploy-docs.yml` publishes to <https://cdot65.github.io/prisma-airs-cli/>. |
| `macos` (`.forgejo/workflows/ci-macos.yml`) | `macos-arm64`, a placeholder for the maintainer's runner `jadzia` | The `linux` checks, `continue-on-error`. Its own workflow, run only on demand (`workflow_dispatch`): Forgejo evaluates a job's `if` only once a runner with its label picks the job up, so a macOS job in `ci.yml` left every push run waiting forever while no runner serves the label. Rename the label and dispatch it once `jadzia` is online and meets the self-hosted runner requirements below. |

Notes:

- **Zig downloads are bounded and verified.** The musl job tries a community mirror,
  then the origin, with a 15-second connection timeout and a 600-second total timeout
  per candidate. Each candidate must match the SHA-256 pinned in the workflow before
  it is promoted or extracted; failed or corrupt candidates never install a tool.
  This avoids spending most of the job on a slow origin download.
- **Tests run unprivileged.** The container runs as root, and root ignores file modes,
  so tests that make a file mode `0000` would fail. The test step creates a user `ci`,
  hands it the workspace and the cargo home, and runs `cargo test` as that user.
- **Docs-only changes skip the Rust steps.** Each job first classifies the change: when
  only `docs/`, `docs-site/` or `*.md` files changed, the `linux` job runs just
  `docs_consistency` and `cli_reference` (the two tests that read the docs) and the `musl`
  job builds nothing; both still report, so branch protection is satisfied.
  `scripts/ci/classify-change.sh` decides, and any doubt runs the Rust steps: renames count
  their old path, every path is examined, and a missing or unrelated base, a git error or
  an empty diff all choose `rust=true` (`tests/ci_scripts.rs`).
- **Self-hosted macOS runner (`jadzia`).** A macOS runner has no container: every job
  runs directly on the host as the runner's user, and a pull request's `build.rs`,
  proc macros and tests run during `cargo build` and `cargo test`. Its workflow never runs
  for pull requests, but a fork pull request could add one that does, so the runner
  setup is what protects the host. Before dispatching `ci-macos.yml` (or adding a
  push trigger to it):
  - register the runner at repository scope for `cdot/prisma-airs-cli` only, never
    for the instance, an organisation or a user;
  - run it as a dedicated non-admin macOS user with no login keychain entries, no
    Developer ID key, no notary profile, no Forgejo token, and no access to
    `~/.prisma-airs`, `~/.config/airs-cli`, `~/.local/state/prisma-airs` or any `*.env`
    file of the maintainer;
  - require maintainer approval before workflows run for pull requests from forks
    (repository settings), or disable fork pull request workflows;
  - keep signing off this runner. If signing moves to `jadzia` ([RELEASING.md](RELEASING.md)),
    it uses a separate runner registration, user and label that only tag pushes on
    `main` reach, never `pull_request`.
- **Caches.** The cargo registry and `target/` are cached per job, keyed on the rustc
  version and `Cargo.lock`. Debug info is off in CI (`CARGO_PROFILE_DEV_DEBUG=0`,
  `CARGO_PROFILE_TEST_DEBUG=0`) to keep `target/` small.
- **Artifacts.** `actions/upload-artifact` v4 refuses hosts other than github.com, and
  the code.forgejo.org fork failed on this instance, so the workflow uses v3.
- **Linux argument limit.** Linux caps a single command-line argument at 128 KiB
  (`MAX_ARG_STRLEN`); macOS only caps the total (1 MiB). A test that passes a larger
  single argument to the CLI passes on macOS and fails on Linux with
  `Argument list too long (os error 7)`. Keep each argument under 128 KiB
  (`tests/scan.rs::request::large_prompt_is_sent_byte_exact` uses 120 KiB).
- **Results.** The Forgejo API lists each job's outcome:
  `GET /api/v1/repos/cdot/prisma-airs-cli/actions/tasks`. Job logs are only in the
  web UI.

## Signal Brief regression coverage

The unreleased Signal Brief replaces the default runtime report and scan-log stub;
see DESIGN §18.11/§18.14 and differences D337–D341. All executable fixtures use
loopback services or synthetic local files, never real tenants:

```sh
cargo test --lib report::signal
cargo test --test signal_sdk --test signal_reads --test signal_offline --test signal_review --test runtime_report
```

`signal_sdk` proves pinned SDK model/request compatibility, not live semantics.
`signal_reads` covers discovery/result/report correlation, request/deadline limits,
credential planes, failed results, sampling and DLP withholding. Offline/review
suites exercise digest binding, extra privacy projection, immutable source files,
private bundles, injection, and agent versus human attribution. Publication unit
checks inject sync/rename failures and races. The existing live tier's runtime
self-tests remain loopback; only the user may enable its real-service gate.

The synthetic examples under `docs/examples/signal/` are generated by the actual
offline CLI. Local visual checks cover desktop/mobile, keyboard disclosure and
print expansion with network requests blocked. Playwright and its previews remain
ignored development tooling; the CLI has no browser/runtime dependency. Independent
phase scores are recorded in SIGNAL_BRIEF_PHASE_REVIEWS.md and final validation
outcomes in SIGNAL_BRIEF_VALIDATION.md.

Runtime history lives in `crates/airs-history-store` since M1 of the runtime MCP plan.
Its unit fixtures share a process-local test gate acquired before
fixture creation and held through cleanup. This prevents independent archive
construction from saturating the same test volume while another case measures
the production 250ms retention deadline. It does not serialize the reader/writer
threads, signals or child processes deliberately used within a case. Child test
processes have separate gates. Poison recovery avoids cascading unrelated failures.
The cancellation-before-send async scenario runs inside a runtime created after
taking the guard, so no synchronous mutex is held across an outer async suspension.
Default workspace scheduling remains the required baseline; separately serialized
emulated/native platform receipts explicitly state their execution conditions.
Earlier parallel fixture failures remain in qualification/audit evidence. Production
deadlines, warning assertions, endpoint/path guards and live-tier restrictions are
unchanged.

The history store's executable tests (`crates/airs-history-store/tests/`) drive
`airs-history-driver`, a test-only binary that keeps the former `runtime sync` and
`runtime history` grammar, JSON output, exit codes and signal handling. The runner's
existing guards apply unchanged; only the driver's `runtime sync` has its default data
directory kept under the sandbox's tmp. The CLI's `runtime scan`, `bulk-scan` and
`resume-poll` run against the sandbox's own data directory, and the unchanged-sandbox
checks prove they retain nothing (`tests/no_local_history.rs`). The automatic-retention
executable tests were retired with the feature. Their receipt domain rules (no payload,
bounded capture, one outcome per run, job-scoped deduplication across resume) are unit
tests in `crates/airs-history-store/src/receipts_tests.rs`. The held-management timeout
test still pins received-request journaling and a partial run, and availability still
determines its exit code.
