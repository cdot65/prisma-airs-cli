# Benchmarks: airs 8.0.0 and the TypeScript CLI 7.3.0

> **Historical.** The benchmark harness (`crates/airs-test-harness/examples/bench.rs`) and the
> TypeScript reference it compared against were removed on 2026-11-08. The results below
> are the last measurement; the reproduction steps no longer apply.

Performance was the main reason for replacing the TypeScript CLI. Every invocation of the
TypeScript CLI pays for starting Node.js, loading and compiling about 900 KiB of JavaScript
and 65 production packages, and running V8, before it reads one argument. `airs` is one
native executable: it starts in a few milliseconds, uses a fraction of the CPU and memory,
and needs no runtime. This page records how large the difference is, how it was measured,
and what it does not cover.

Every number comes from `crates/airs-test-harness/examples/bench.rs`. The Rust side is the
release binary (`target/release/airs`). The TypeScript side is the pinned reference build
of 7.3.0 run as `node .reference/prisma-airs-cli/dist/cli/index.js`, which is what the
installed 7.x `airs-cli` command runs. The raw data of the run shown here is written to
`target/bench/<name>.json` (not checked in).

## Summary

Loopback run of 2026-10-23 (airs 8.0.0 against TypeScript 7.3.0; 30 timed runs per scenario
and CLI; medians; Apple M5 Pro):

| What | airs 8.0.0 | TypeScript 7.3.0 | Factor |
|---|---:|---:|---:|
| Start-up (`--version`) | 6.3 ms | 80.6 ms | 13x |
| Local commands (`--help`, `tenant list`, `tenant read`) | 8.7–8.9 ms | 82.4–85.9 ms | 9.3–9.9x |
| One API request (one profiles page, one scan) | 17.5–17.7 ms | 96.8–98.9 ms | 5.5–5.6x |
| `doctor` (4 and 5 requests) | 18.5 ms | 117 ms | 6.3x |
| `--all` walks over 2,000 rows (11–21 requests) | 29.8–44.4 ms | 118–130 ms | 2.9–4.0x |
| 50 sequential scans | 0.85 s | 4.91 s | 5.8x |
| CPU time (user + sys) | 3.5–34 ms | 95–207 ms | 6.1–27x less |
| Peak memory (RSS) | 6.5–71.8 MiB | 105.8–173.6 MiB | 2.0–16x less |
| Installed size (macOS arm64) | 31.1 MiB, 1 file | 219.6 MiB: 103.2 MiB of packages (3,104 files) and a 116.5 MiB `node` | 7.1x smaller |

- **Node's compile cache does not close the gap.** With `NODE_COMPILE_CACHE` set, the
  TypeScript CLI is 10–12 ms faster per invocation; the factors become 2.7–11x.
- **Both CLIs send the same requests** in every scenario except `doctor`, whose checks
  differ (Caveats).
- **On a real tenant the saving is a constant, not a factor.** A management read spends
  600–800 ms waiting for the service, so the 80 ms that `airs` saves per command is
  1.0–1.2x there (live results, measured on 0.1.0). The factors above apply in full to
  local commands, to scripts that run the CLI many times, and to CPU and memory, which the
  network does not hide.
- **8.0.0 is slower than the first Rust release, and still far ahead.** The binary grew from
  9.6 MiB (0.1.0, 10 commands) to 31.1 MiB (8.0.0, every command group), and a parsed
  command now costs about 2.5 ms more and an API command about 4 ms more than in 0.1.0
  (What changed since 0.1).

## Footprint

### What a user installs

| Item | airs 8.0.0 | TypeScript 7.3.0 |
|---|---:|---:|
| Program | one executable: 31.07 MiB (macOS arm64, signed), 38.65 MiB (Linux x86_64, static musl), 30.63 MiB (Linux arm64, static musl) | 0.88 MiB of JavaScript (`dist/`, 5 files) |
| Runtime | none. macOS links only system libraries (Security, SystemConfiguration, CoreFoundation, libiconv, libSystem); the Linux builds are fully static | Node.js: the `node` v24.21.0 binary is 116.47 MiB |
| Dependencies | compiled in: 189 crates in the normal dependency graph (236 packages in `Cargo.lock`, with dev and build) | 65 production npm packages, installed beside it |
| Download | 12.96 / 14.49 / 13.33 MiB release tarballs; npm: a 13.4–14.6 MiB platform package plus an 8 KiB launcher | npm package and its dependencies |
| `npm install -g` (macOS arm64, measured) | 31.13 MiB in 11 files (the launcher and one platform package) | 103.17 MiB in 3,104 files, plus Node |
| Total on disk | 31.1 MiB | 219.6 MiB |

The largest TypeScript dependencies are `@cdot65/prisma-airs-sdk` (26.0 MiB), `pdf-lib`
(22.7 MiB), `docx` (19.3 MiB) and `@img/sharp-libvips-darwin-arm64` (17.4 MiB). `pdf-lib`,
`docx` and `sharp` serve only `runtime dlp generate`, a command 8.0 dropped (MIGRATION
§2.2). A user who already runs Node for other work does not pay its 116 MiB for the
TypeScript CLI alone; the CLI's own share is then 103 MiB against 31 MiB.

The npm channel keeps a Node requirement only for its 8 KiB launcher, which starts the
native binary (DESIGN §21.16). Homebrew, crates.io and the tarballs need no Node at all.

### How the executable grew

Each Rust release added a command group of the TypeScript CLI. The macOS arm64 binary
(symbols stripped, `lto = "thin"`, `codegen-units = 1`), read from each release's tarball:

| Release | Added | Binary | Tarball |
|---|---|---:|---:|
| 0.1.0 | tenants, `doctor`, profile and topic reads, scans | 9.63 MiB | 3.88 MiB |
| 0.2.0 | pretty and table output, `--debug`, Linux builds | 9.75 MiB | 3.91 MiB |
| 0.4.0 | runtime writes | 10.62 MiB | 4.32 MiB |
| 0.5.0 | bulk scanning | 11.25 MiB | 4.60 MiB |
| 0.6.0 | Red Team | 15.79 MiB | 6.57 MiB |
| 0.7.0 | DLP | 17.94 MiB | 7.48 MiB |
| 0.8.0 | Model Security | 19.52 MiB | 8.16 MiB |
| 0.9.0 | runtime extras, the HTML reports | 21.39 MiB | 8.97 MiB |
| 0.10.0 | AI Gateway | 30.47 MiB | 12.69 MiB |
| 0.11.0 | Supplychain | 30.82 MiB | 12.85 MiB |
| 1.0.0 | the `airs` name | 30.93 MiB | 12.90 MiB |
| 8.0.0 | TypeScript 7.3.0's directional profiles, the `airs-cli` name | 31.07 MiB | 12.96 MiB |

From 0.1.0 to 8.0.0 the binary grew 3.2x while the CLI grew from 10 to 277 leaf commands.
It now covers the whole TypeScript CLI in one file that is still 7x smaller than the
TypeScript install. The largest steps are the SDK feature sets each command group compiles
in (Red Team in 0.6, AI Gateway in 0.10).

## What changed since 0.1

The first benchmark (2026-10-17) measured `airs-cli-rs` 0.1.0, which implemented 10
commands, against TypeScript 7.2.1. The same scenarios on 8.0.0:

| Scenario | 0.1.0 median | 8.0.0 median | 0.1.0 RSS | 8.0.0 RSS |
|---|---:|---:|---:|---:|
| `--version` | 6.27 ms | 6.27 ms | 6.2 MiB | 6.5 MiB |
| `--help` | 6.27 ms | 8.71 ms | 7.2 MiB | 16.5 MiB |
| `tenant list` | 6.37 ms | 8.79 ms | 7.9 MiB | 17.5 MiB |
| `doctor` | 13.7 ms | 18.5 ms | 38.5 MiB | 57.7 MiB |
| one profiles page | 13.2 ms | 17.6 ms | 38.6 MiB | 57.3 MiB |
| one scan | 13.4 ms | 17.5 ms | 38.5 MiB | 57.3 MiB |
| profiles `--all` (2,000) | 41.0 ms | 44.4 ms | 48.0 MiB | 71.8 MiB |

- `--version` is answered before the argument parser is built and has not moved.
- Every parsed command now costs about 2.5–3 ms and 9–10 MiB more. That is consistent with
  building the parser for 277 leaf commands where 0.1 had 10;
  it has not been profiled further.
- Commands that reach the API cost about 4 ms and 19 MiB more. `doctor` also makes one more
  request (the `Gateway API` check, 0.10).
- The TypeScript reference became 9–13 ms faster between the runs (7.2.1 on a busier
  machine, then 7.3.0), so the factors on this page are lower than the first run's
  (14–15x then, 13x now for start-up) for both reasons.

## Method

The scenarios are the commands both CLIs share on the hot path: start-up, tenant reads,
`doctor`, profile and topic reads (one page, and `--all` over 2,000 rows) and scans. They
cover the work every command does (process start, argument parsing, tenant loading, the
HTTP client, JSON handling, output); no command group of either CLI is excluded on purpose.

### Loopback mode

1. Each scenario gets a fresh `FixtureServer` (127.0.0.1) and `Sandbox`. The tenant
   config is `FixtureConfig::standard`, so every endpoint points at the fixture and the
   harness loopback guard (`Sandbox::assert_safe_for_ts`) checks every TypeScript run.
2. Datasets: "small" has 3 profile rows (2 names) and 3 topic rows. "bulk" has 2,000
   profiles and 2,000 topics with unique names. The bulk profiles walk is 20 pages of
   100 (`latest=true`). The topics walk is 10 pages of 200.
3. Verification: each scenario runs once per CLI through the harness `run`, with its
   timeout, secret leak check and unchanged-sandbox check. The run must exit 0 with
   the expected output: a version string, `Usage`, the tenant, a `mgmtTsgId` row, the
   doctor probe checks, the listed names, exactly 2,000 rows, the looked-up profile,
   or an `allow` verdict. If either CLI fails, the benchmark stops.
4. Timing: the runs go through a `python3 -I` helper started once. For each run it
   receives the argv, the environment and the cwd. It records `time.perf_counter_ns()`,
   starts the CLI with `subprocess.Popen` (stdin `/dev/null`, stdout and stderr pipes
   drained by threads), and reaps it with `os.wait4`. The wall time runs from just
   before `Popen` to `wait4` returning. User time, system time and `ru_maxrss` (peak
   resident set size, in bytes on macOS) come from that one child's rusage. The
   environment is exactly the parity tests' scrubbed environment
   (`airs_test_harness::child_environment`): `PATH`, `HOME`, `TMPDIR`,
   `PRISMA_AIRS_TENANTS_PATH`, `NO_COLOR=1`, `NO_PROXY`.
5. Every timed run is checked again: exit 0, the scenario's expected output, no fixture
   secret in either stream, no sandbox file changed outside `tmp/`, and the number of
   requests the fixture recorded.
6. Order: 3 untimed warm-up rounds, then 30 timed rounds. Each round runs every CLI
   once and rotates which goes first, so slow drift affects all CLIs alike. The batch
   scenario uses 5 timed rounds of 50 sequential scans per CLI. Its numbers are per
   batch: the sum of the 50 wall and CPU times, and the largest RSS.
7. Statistics: min, median, mean, nearest-rank p95, max and sample standard deviation.
   A ratio is the TypeScript median divided by the Rust median.
8. Spawn floor: the same helper starting `/usr/bin/true` takes 1.64 ms (median, p95
   1.79 ms). This fixed cost is included in every wall time and is not subtracted.
   Subtracting it would raise the short-command ratios: for `--version`,
   (80.6 − 1.6) / (6.27 − 1.6) is about 17x instead of 13x.
9. The `airs-cli+cache` variant runs the same TypeScript CLI with
   `NODE_COMPILE_CACHE=<sandbox>/tmp/node-compile-cache`. The CLI does not enable
   Node's compile cache itself, and the scrubbed environment does not set it, so the
   plain `airs-cli` rows have no compile cache. The verification and warm-up runs fill
   the cache before the timed runs.

### Live mode

`--live` times only read-only commands against one real tenant: `tenant read <name>
--output json`, `doctor --output json`, `runtime profiles list --output json`,
`runtime topics list --output json` and `runtime profiles get <first listed>
--output json`. It never scans. It runs only with `AIRS_RS_LIVE=1` and
`AIRS_E2E_TENANT=<name>`, and refuses otherwise
(`live::LiveBenchSettings`, unit-tested in `live_bench_refuses_without_the_opt_in`).
The flow is `live::run_live_bench`, which reuses the live tier's session
(`tests/live.rs`, TESTING §6.2):

- the real registry is read only to copy the selected entry into an isolated registry
  in a temp sandbox;
- both CLIs run with a cleared environment;
- every output is searched for the config's secrets in each encoded form and for
  JWT-shaped text;
- the real registry, the real config and the isolated registry are fingerprinted
  before the first run and re-checked after every run;
- the session's scan budget is 0.

There is 1 untimed warm-up per check and CLI, then 5 timed rounds by default (at most
20), alternating which CLI goes first. The output (`target/bench/live.json`) holds only
check labels, exit codes and durations: no tenant, profile or topic names, IDs or
bodies. The wall time is measured by polling the child every 1 ms. The rusage meter is
not used in live mode. `tests/live.rs::live_bench_self_test_against_fixture` runs the
whole flow against the fixture server and checks that the report is free of names,
IDs, paths and secrets.

## Environment

| Item | Value |
|---|---|
| Date (UTC) | 2026-10-23T00:21:22Z |
| Machine | Apple M5 Pro, 18 cores (6 Super, 12 Performance), 64 GiB, on AC power |
| OS | macOS 27.0.1 (26A434), Darwin 27.0.0 |
| airs | 8.0.0, built from `c5c21aa` (the `v8.0.0` release commit, clean tree); release profile `lto = "thin"`, `codegen-units = 1`, `strip = "symbols"`; binary sha256 `6eee7fde…dedaff` |
| rustc | 1.99.0 (b940084d7 2026-09-28) |
| TypeScript CLI | 7.3.0, reference commit `7a54e46eee4f7914d610bc78d2f98dd7de09fbfe` (tag `v7.3.0`) |
| node | v24.21.0 |
| Meter | Python 3.13.15 (`-I`) |
| Background load | Not idle. Load average 5.2 (1 min) at the start and 3.2 at the end; the summed `ps` %CPU of all processes was 187 at the start and 101 at the end (100 = one busy core). |

## Reproduce

```sh
export PATH="$HOME/.cargo/bin:$PATH"
scripts/build-reference-cli.sh --verify     # the pinned TypeScript reference, node 24 on PATH
cargo build --release                       # the benchmark refuses an older binary
cargo run --release -p airs-test-harness --example bench -- --ts-compile-cache --name loopback
```

Options: `--iterations N` (default 30), `--warmup N` (3), `--batch-size N` (50),
`--batch-iterations N` (5), `--filter TEXT`, `--name NAME` (letters, digits, `-` and `_`;
the output file name), `--no-static`, `--show-output`, `--allow-stale`.
`AIRS_BENCH_PYTHON` overrides the interpreter (default: what `python3` on `PATH` resolves
to). The run takes about 3 minutes and writes `target/bench/<name>.json` and
`target/bench/<name>.md`.

Live mode (only the maintainer runs it):

```sh
AIRS_RS_LIVE=1 AIRS_E2E_TENANT=<name> \
  cargo run --release -p airs-test-harness --example bench -- --live [--iterations 5]
```

## Loopback results

Medians of 30 runs (batch: 5 batches of 50 scans). "requests" is the number of requests
the fixture recorded per run, the same for every run. `TS+cache` is the TypeScript CLI with
Node's compile cache filled (Method item 9).

| Scenario | Rust ms | TS ms | TS/Rust wall | TS+cache ms | TS+cache/Rust | Rust CPU ms | TS CPU ms | TS/Rust CPU | Rust RSS MiB | TS RSS MiB | TS/Rust RSS | requests rs / ts |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| `--version` | 6.27 | 80.6 | 13x | 69.8 | 11x | 3.51 | 94.6 | 27x | 6.5 | 105.8 | 16x | 0 / 0 |
| `--help` | 8.71 | 85.9 | 9.9x | 74.3 | 8.5x | 5.77 | 100 | 17x | 16.5 | 108.1 | 6.5x | 0 / 0 |
| `tenant list --output json` | 8.79 | 82.4 | 9.4x | 71.1 | 8.1x | 5.93 | 101 | 17x | 17.5 | 109.2 | 6.2x | 0 / 0 |
| `tenant read --output json` | 8.92 | 82.7 | 9.3x | 71.7 | 8.0x | 5.98 | 101 | 17x | 17.8 | 109.4 | 6.2x | 0 / 0 |
| `doctor --output json` | 18.5 | 117 | 6.3x | 106 | 5.7x | 14.5 | 140 | 9.7x | 57.7 | 143.4 | 2.5x | 4 / 5 |
| `runtime profiles list --output json` (1 page) | 17.6 | 98.9 | 5.6x | 87.7 | 5.0x | 14.0 | 121 | 8.6x | 57.3 | 117.6 | 2.1x | 2 / 2 |
| `runtime profiles list --all --output json` (2,000) | 44.4 | 130 | 2.9x | 119 | 2.7x | 33.7 | 207 | 6.1x | 71.8 | 173.6 | 2.4x | 21 / 21 |
| `runtime topics list --all --output json` (2,000) | 29.8 | 118 | 4.0x | 108 | 3.6x | 24.2 | 171 | 7.1x | 61.2 | 160.1 | 2.6x | 11 / 11 |
| `runtime profiles get profile-1000 --output json` (of 2,000) | 38.4 | 126 | 3.3x | 116 | 3.0x | 31.5 | 205 | 6.5x | 67.4 | 169.6 | 2.5x | 21 / 21 |
| `runtime scan` (pretty) | 17.5 | 97.2 | 5.6x | 86.3 | 4.9x | 14.0 | 119 | 8.5x | 57.1 | 117.5 | 2.1x | 1 / 1 |
| `runtime scan --output json` | 17.5 | 96.8 | 5.5x | 85.9 | 4.9x | 14.0 | 118 | 8.5x | 57.3 | 117.5 | 2.1x | 1 / 1 |
| `runtime scan --output json --response …` | 17.7 | 97.7 | 5.5x | 86.7 | 4.9x | 14.1 | 119 | 8.5x | 57.3 | 117.4 | 2.0x | 1 / 1 |
| 50 sequential `runtime scan --output json` (per batch) | 851 | 4,906 | 5.8x | 4,318 | 5.1x | 685 | 5,988 | 8.7x | 58.2 | 117.7 | 2.0x | 50 / 50 |

Wall-time spread (ms):

| Scenario | CLI | min | median | mean | p95 | max | stddev |
|---|---|---:|---:|---:|---:|---:|---:|
| `--version` | airs | 5.73 | 6.27 | 6.28 | 6.78 | 6.83 | 0.24 |
| | TypeScript | 78.5 | 80.6 | 80.7 | 83.1 | 86.5 | 1.54 |
| `--help` | airs | 8.38 | 8.71 | 8.71 | 9.06 | 9.09 | 0.16 |
| | TypeScript | 83.5 | 85.9 | 85.8 | 87.7 | 88.5 | 1.22 |
| `tenant list` | airs | 8.58 | 8.79 | 8.80 | 9.07 | 9.11 | 0.14 |
| | TypeScript | 80.8 | 82.4 | 82.6 | 84.7 | 87.2 | 1.24 |
| `tenant read` | airs | 8.56 | 8.92 | 8.93 | 9.31 | 9.37 | 0.18 |
| | TypeScript | 80.9 | 82.7 | 82.8 | 84.4 | 84.9 | 0.97 |
| `doctor` | airs | 18.0 | 18.5 | 18.5 | 19.0 | 19.5 | 0.31 |
| | TypeScript | 116 | 117 | 117 | 119 | 122 | 1.34 |
| profiles list, 1 page | airs | 17.3 | 17.6 | 17.6 | 17.9 | 19.0 | 0.29 |
| | TypeScript | 97.4 | 98.9 | 98.9 | 101 | 102 | 1.00 |
| profiles list `--all` (2,000) | airs | 44.0 | 44.4 | 44.8 | 45.4 | 55.2 | 1.98 |
| | TypeScript | 127 | 130 | 131 | 135 | 147 | 3.41 |
| topics list `--all` (2,000) | airs | 29.4 | 29.8 | 30.0 | 31.7 | 32.4 | 0.62 |
| | TypeScript | 116 | 118 | 119 | 126 | 133 | 3.40 |
| profiles get (of 2,000) | airs | 37.7 | 38.4 | 38.4 | 39.2 | 39.3 | 0.39 |
| | TypeScript | 124 | 126 | 127 | 129 | 130 | 1.53 |
| scan, pretty | airs | 17.3 | 17.5 | 17.5 | 17.7 | 17.8 | 0.13 |
| | TypeScript | 95.3 | 97.2 | 97.1 | 98.4 | 98.4 | 0.75 |
| scan, json | airs | 17.2 | 17.5 | 17.5 | 18.0 | 18.0 | 0.20 |
| | TypeScript | 95.7 | 96.8 | 97.1 | 98.9 | 100 | 0.97 |
| scan, prompt + response | airs | 17.3 | 17.7 | 17.6 | 17.8 | 17.9 | 0.15 |
| | TypeScript | 96.4 | 97.7 | 97.8 | 99.8 | 99.9 | 0.92 |
| 50 scans (per batch, n = 5) | airs | 847 | 851 | 853 | 861 | 861 | 5.20 |
| | TypeScript | 4,854 | 4,906 | 4,896 | 4,947 | 4,947 | 37.5 |

The `airs` spread is tight: its p95 is within 2 ms of its median in every single-command
scenario. User and system CPU, the compile-cache rows and every raw sample are in
`target/bench/<name>.json`. The TypeScript CPU time is higher than its wall time because V8
and libuv use helper threads.

## Caveats

- **Loopback isolates CLI overhead.** The fixture answers from memory on `127.0.0.1` over
  plain HTTP. A real tenant adds network latency and TLS to every request, for both CLIs.
  For commands dominated by network time the absolute saving per invocation (about 80 ms
  for a scan here) is the useful number, and the ratio shrinks. TLS handshake cost was not
  measured: the two CLIs use different TLS stacks.
- **The fixture's own time is in every number.** The fixture server runs in the benchmark
  process; its per-request cost is the same for both CLIs.
- **doctor does different work.** The TypeScript doctor runs 11 checks and `airs` 9 (D19,
  D331). That is 5 requests for TypeScript against 4 for `airs`; the shared checks pass on
  both.
- **Output formats.** Management and tenant scenarios use `--output json` on both CLIs. The
  TypeScript scan always prints the pretty verdict, even with `--output json` (D5), so the
  json scan rows compare Rust JSON with TypeScript pretty output.
- **Warm runs only.** All timed runs follow a verification run and 3 warm-ups, so the file
  cache is warm and, for the cache variant, the compile cache is full. The first invocation
  after a reboot or cache eviction was not measured; a cold Node start also has to page in
  its 116 MiB binary and its packages.
- **Peak RSS** is `ru_maxrss`: the resident peak of the process, including touched pages
  of the node binary or the Rust binary. It is not unique heap use.
- **Measurement overhead.** The Python helper's spawn floor (1.64 ms) is in every wall
  time; see Method item 8. stdout and stderr are pipes, as in the harness.
- **Background load.** The machine was running other work during the run (see
  Environment). The medians and the narrow spreads agree.
- **Single machine.** One Apple Silicon Mac, one node version (24), one build of each CLI.
  The Linux builds were not benchmarked.

## Live (read-only) results (0.1.0)

The live tier is run by the maintainer, and it has not been repeated for 8.0.0. These are
the 0.1.0 runs; the CLI overhead they show is the loopback overhead above, which 8.0.0
keeps within a few milliseconds. Two runs on 2026-10-17, from a home network, against one
production tenant: 5 and then 10 iterations per CLI, in rotating order, on the same machine
as the loopback run. Times are milliseconds.

The 10-iteration run:

| Check | CLI | exits | min | median | mean | p95 | max |
|---|---|---|---:|---:|---:|---:|---:|
| tenant read --output json | airs-cli-rs 0.1.0 | 0x10 | 3.96 | 5.32 | 4.80 | 5.35 | 5.35 |
| tenant read --output json | airs-cli 7.2.1 | 0x10 | 76.9 | 78.4 | 78.5 | 80.9 | 80.9 |
| doctor --output json | airs-cli-rs 0.1.0 | 0x10 | 874 | 4807 | 4275 | 5682 | 5682 |
| doctor --output json | airs-cli 7.2.1 | 0x10 | 1318 | 1411 | 1417 | 1516 | 1516 |
| runtime profiles list --output json | airs-cli-rs 0.1.0 | 0x10 | 693 | 747 | 745 | 787 | 787 |
| runtime profiles list --output json | airs-cli 7.2.1 | 0x10 | 745 | 777 | 787 | 836 | 836 |
| runtime topics list --output json | airs-cli-rs 0.1.0 | 0x10 | 657 | 701 | 704 | 766 | 766 |
| runtime topics list --output json | airs-cli 7.2.1 | 0x10 | 702 | 736 | 745 | 789 | 789 |
| runtime profiles get <first> --output json | airs-cli-rs 0.1.0 | 0x10 | 709 | 780 | 772 | 843 | 843 |
| runtime profiles get <first> --output json | airs-cli 7.2.1 | 0x10 | 789 | 834 | 835 | 905 | 905 |

Medians from the 5-iteration run:

| Check | airs-cli-rs 0.1.0 | airs-cli 7.2.1 |
|---|---:|---:|
| tenant read | 5.34 | 79.6 |
| doctor | 854 | 1465 |
| profiles list | 732 | 818 |
| topics list | 674 | 826 |
| profiles get | 758 | 855 |

What the live numbers show:

- **Local commands keep the full gap.** `tenant read` was 5.3 ms against 78–80 ms
  (15x).
- **Network-bound commands are dominated by the service.** A management read is an
  OAuth token request (about 350–400 ms) plus one or more API requests (about
  220–250 ms each). The Rust CLI saves a fairly constant 30–100 ms per command, which
  is the process start-up and runtime overhead also seen on loopback. In ratio terms
  that is 1.0–1.2x.
- **Do not read `doctor` as a speed result.** When the Rust doctor runs back to back,
  the Scanner API probe's `GET /v1/scan/results` is answered with HTTP 429. Both CLIs
  were timed with per-request instrumentation, alternating Rust and TS doctors:
  - The first Rust doctor in a quiet period took 0.86 s, with all three requests
    answered 200 in 180–400 ms.
  - Later ones got 429 on the scanner probe. The SDK retried the 429 up to four times
    with backoff until a 200 arrived or the 5 s probe budget ran out, so the run took
    4.9–6.1 s.
  - The TS doctor does not retry a 429 and reports it as a pass (D20), so it stayed at
    1.3–1.8 s, with one 3.2 s run.
  - Every run of both CLIs exited 0. A persistent 429 is a warning in the Rust doctor
    (DESIGN §9), not a failure.
  - The Rust median of 4.8 s in the 10-iteration run therefore measures the
    rate-limit retry policy, not CLI speed. A preflight that does not retry would
    report the warning at once (next section).

## Doctor and rate limits

Up to 0.1, the Scanner API and Management OAuth probes ran with the SDK's normal retry
policy inside one 5 s budget, and that policy retries 429/502/503/504. Under a rate
limit, the doctor therefore spent up to the whole budget waiting before it warned.
That was slow, and the retries added load to an endpoint that had already said "slow
down".

Since 0.2 the probes make one attempt each (`RequestOptions::with_retries(0)`, D49),
so a 429 or 5xx is classified at once. The rate-limited case therefore costs one
request and returns in about the time of an unthrottled doctor (about 0.9 s live). The
live numbers above predate this change.
