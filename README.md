# airs-cli

airs-cli (the command `airs`) is a native Rust CLI for Prisma AIRS. It replaces the
TypeScript Prisma AIRS CLI, which lived in this repository through 7.x (its sources stay
in the git history and the `v7.*` tags). It is built on the published `scm-sdk` crate and
is published on crates.io as `airs-cli`. The same program also answers to `airs-cli`, the
TypeScript CLI's command name, so 7.x scripts keep working (from 8.0), and to
`airs-cli-rs`, the Rust 0.x command name; every name prints the same output, and help and
hints say `airs`. It keeps its own tenant store (TOML files under `~/.config/airs-cli/`)
and creates, edits, selects and deletes tenants itself (see
[Upgrading from the TypeScript CLI](#upgrading-from-the-typescript-cli-7x)).

[docs/DESIGN.md](docs/DESIGN.md) is the behavioural contract. Its §12 lists every
intentional difference from the TypeScript CLI. From 0.9.1 (scm-sdk 0.2.2) no command
refuses an answer on a string format (`uuid`, `date-time`, `email`, `url` and the like):
where the TypeScript CLI and 0.9.0 fail the read, `airs` reads and prints the answer
unchanged (D306).

## Why Rust: performance

Speed and footprint are the reason this CLI was rewritten. Every run of the TypeScript CLI
first starts Node.js, then loads and compiles its JavaScript and 65 production packages,
before it reads its first argument. `airs` is one native executable with no runtime: it
starts in about 6 ms, and its p95 stays within 2 ms of its median.

Measured on 2026-10-23, `airs` 8.0.0 against the TypeScript CLI 7.3.0 (Apple M5 Pro; medians
of 30 runs against a loopback fixture; [docs/BENCHMARKS.md](docs/BENCHMARKS.md)):

| Workload | `airs` 8.0.0 | TypeScript 7.3.0 | Faster |
|---|---:|---:|---:|
| Start-up (`airs --version`) | 6.3 ms | 80.6 ms | **13x** |
| Local commands (`--help`, `tenant list`, `tenant read`) | 8.7–8.9 ms | 82.4–85.9 ms | **9.3–9.9x** |
| One API call (a scan, one profiles page) | 17.5–17.7 ms | 96.8–98.9 ms | **5.5–5.6x** |
| `doctor` | 18.5 ms | 117 ms | **6.3x** |
| Paging 2,000 profiles or topics (`--all`) | 29.8–44.4 ms | 118–130 ms | **2.9–4.0x** |
| 50 scans in a row | 0.85 s | 4.91 s | **5.8x** |

| Resources | `airs` 8.0.0 | TypeScript 7.3.0 | Less |
|---|---:|---:|---:|
| CPU time per command | 3.5–34 ms | 95–207 ms | **6.1–27x** |
| Peak memory per command | 6.5–72 MiB | 106–174 MiB | **2.0–16x** |
| Installed on disk (macOS arm64) | 31.1 MiB, one executable | 219.6 MiB: 103 MiB in 3,104 files, plus the 116 MiB Node.js runtime | **7.1x** |
| Runtime required | none | Node.js | |

- **The executables.** `airs` ships as one binary per platform: 31.07 MiB on macOS arm64
  (signed and notarised), 38.65 MiB on Linux x86_64 and 30.63 MiB on Linux arm64 (both
  static musl, no shared libraries). The release tarballs are 13.0–14.5 MiB. The binary grew
  from 9.6 MiB in 0.1.0 (10 commands) to 31.1 MiB in 8.0.0 (277 commands, every TypeScript
  command group), in steps that follow the ported groups; BENCHMARKS "How the executable
  grew" has the table.
- **What the growth cost.** Against 0.1.0, a parsed command is about 2.5 ms slower and an API
  command about 4 ms slower, which leaves every factor above in place.
- **On a real tenant** a management read waits 0.7–0.8 s for the service, so the 80 ms that
  `airs` saves per command shows as a constant, not a multiple. Scripts and agents that run
  the CLI hundreds of times, local commands, and the CPU and memory a host spends on them
  get the full factors.
- **Node's compile cache** (`NODE_COMPILE_CACHE`) makes the TypeScript CLI 10–12 ms faster
  per run; `airs` is still 2.7–11x faster than that.

These figures were measured on 2026-10-23; the benchmark harness that compared both CLIs
was removed with the TypeScript parity tests (2026-11-08).

## Status

**8.2.0 is the current release** ([release notes](docs/RELEASE-NOTES/8.2.0.md)). It
carries the 8.1 work (the AIRS Signal Brief, scan-log discovery, application-bound scanner
results and report lookups, private bundles and offline attributed review) and the optional
`airs --mcp` client for the separately deployed runtime MCP service, which holds persistent
history. The CLI itself keeps no history and contains no SQLite. The service's acceptance
is tracked in [the phase record](docs/RUNTIME_MCP_PHASE_REVIEWS.md).
The published stable version remains 8.0.0; these changes have not been published.
See [test builds](docs/SIGNAL_BRIEF_TEST_BUILDS.md) and
[release candidate notes](docs/RELEASE-NOTES/8.1.0-rc.4.md).

Version 8.2.0 covers milestones 1–3, the M3.5 foundations, M4t tenant writes, M4 runtime
writes, M5 bulk scanning, M6a Red Team, M6b DLP, M6c Model Security, M6d Runtime extras, M6e
Gateway and M6f Supplychain, renames the executable to `airs` (1.0, DESIGN §21;
[docs/MILESTONES.md](docs/MILESTONES.md)), and replaces the TypeScript CLI (8.0).

**8.0.0.** This repository held the TypeScript CLI through 7.x; the Rust CLI moved
in from `cdot/prisma-airs-cli-rs` (at `a9d7864`) and replaces it. 8.0.0 follows the Rust
CLI's 1.0.0 and is the first release from this repository, with one version on
every channel (crates.io, npm, Homebrew, the Forgejo release and the docs site): it adds
the `airs-cli` name and the npm channel described below, and ports the TypeScript CLI
7.3.0 changes (directional runtime security profiles, accepted against a live tenant, see
[Directional profiles](#directional-profiles-80)) on `scm-sdk` 0.2.4, the
TypeScript SDK 0.35.0 overlay (`=0.2.4` from crates.io, DESIGN §15.25). There is no 1.0.1
or 1.1.0 release.

| Milestone | Commands |
|---|---|
| M1 tenant loading and diagnostics | `tenant list`, `tenant path`, `tenant read`, `tenant get`, `doctor` |
| M2 read-only management | `runtime profiles list/get`, `runtime topics list/get` |
| M3 scanning | `runtime scan` |
| M3.5 foundations | `pretty`, `table` and `json` output with the TypeScript CLI's layouts and colours for every command; `--debug` (a redacted API log, status and headers only); Linux musl builds |
| M4t tenant writes | its own TOML tenant store; `tenant create`, `set`, `unset`, `switch` and `delete` |
| M4 runtime writes | `runtime topics create/update/delete/apply/revert/eval/sample`, `runtime profiles create/update/delete/cleanup` |
| M5 bulk scanning | `runtime bulk-scan`, `runtime resume-poll` |
| M6a Red Team reads | the red team reads: `redteam list/status/categories/languages`, `eula status/content`, `instances get`, `prompt-sets list/get/download`, `prompts list/get`, `properties list/values`, `targets list/get/profile/metadata/templates/error-logs`, `adapter list/get`, `network-broker channels list/get`, `network-broker stats` |
| M6a Red Team writes and scans | the red team writes and the scan: `redteam scan` (with its bounded wait), `abort`, `eula accept`, `registry-credentials`, `instances create/update/delete`, `devices create/update/delete`, `prompt-sets create/update/archive/upload`, `prompts add/update/delete`, `properties create/add-value`, `targets create/update/delete/probe/update-profile/validate-auth/init`, `adapter create/update/delete/validate`, `network-broker channels create/update` |
| M6a Red Team reports and backups | `redteam report\|dashboard <jobId>` (one scan's report), `redteam report` (the HTML environment report), `targets backup`, `targets restore` |
| M6b DLP | `runtime dlp filtering-profiles list/get/replace`, `patterns list/get/create/replace/patch/delete`, `profiles list/get/create/replace/patch` (and `profiles delete`, which explains why the API cannot delete a profile), `dictionaries list/get/create/replace/patch/delete`, `runtime dlp backup`, `runtime dlp restore` |
| M6c Model Security | `model-security groups list/get/create/update/delete`, `rules list/get`, `rule-instances list/get/update`, `scans list/get/create/evaluations/evaluation/violations/violation/files`, `models list/get/versions/version/files`, `labels keys/values/add/set/delete`, `pypi-auth`, `install` (uv only, with `--fetch-uv`) |
| M6d runtime extras | `runtime api-keys list/create/regenerate/delete`, `customer-apps list/get/update/delete/consumption`, `deployment-profiles list`, `scan-logs query/results/reports`, `dashboard applications/application/application-violations/top-applications/violations-trend/apps-list`, `sessions chart/list/get/transaction/scan-content`, `runtime report` (the AIRS Signal Brief), `runtime profiles backup`, `runtime profiles restore` |
| M6e Gateway reads | `gateway workspaces list/get`, `gateway scopes list/get`, every `gateway telemetry` read (`cost`, `cache summary/trend`, `error-category-trends`, `error-trends`, `errors`, `feedback distribution/models/trend/weighted`, `filter-boundaries`, `group-by`, `grouped-errors`, `latency`, `logs list`, `requests`, `rescued-retries`, `tokens`, `user-trends`, `users`), every inventory and admin-guardrail read (`api-keys service/user list/get`, `audit-logs list`, `configs list/get/versions`, `deployments list/get/ping`, `guardrails list/get/catalog`, `integrations list/get/providers/models list/workspaces list`, `mcp integrations list/get/capabilities list/metadata/workspaces list`, `organisations info/self get/auth-settings get`, `plugins list`, `providers list/get`, `admin-guardrails list/get/mcp-servers list`) |
| M6e Gateway writes | `gateway workspaces create/update/archive`, `scopes create/bind/delete`, and the `configs`, `guardrails`, `providers`, `api-keys`, `deployments`, `integrations`, `mcp integrations`, `organisations`, `plugins` and `admin-guardrails` creates, updates, deletes, rotations, sets, syncs and upserts (one-time credentials only to a new `--secret-output` file) |
| M6e Gateway inference and report | `gateway inference chat/responses/embeddings`, `gateway report` (alias `dashboard`, the HTML environment report) |
| M6e Gateway doctor | the doctor's `Gateway API` check |
| M6f Supplychain (0.11) | `supplychain scans list/vulnerabilities`, `supplychain stats`, `supplychain rules list`, `supplychain report` (the HTML environment report) |
| 8.0 TypeScript 7.3.0 port | directional security profiles on `runtime profiles create/update` (`--direction`, `--ai-profile-index`, `--[no-]enable-full-conversation-inspection`, `--no-mask-data-inline`, `--no-mask-data-in-storage`) and `runtime topics apply/revert`; the profile views, the runtime report and profile backup and restore read directions (supported, for single-entry profiles; see the known limitations below) |

`markdown`, `csv` and `yaml` output are dropped, and so is `runtime dlp generate`. The
TypeScript `aigateway` group is `gateway` here, with no `aigateway` alias, and `agentguard` is
`supplychain`, with no `agentguard` alias.
Prebuilt, signed binaries are attached to each
[Forgejo release](https://git.cdot.io/cdot/prisma-airs-cli/releases). The user
documentation is at <https://cdot65.github.io/prisma-airs-cli/> (built from `docs-site/`).

RC4 adds named application scanner routing and preserves positive management
detector evidence when scanner detail is unavailable.
Use `airs tenant scanners bind-default TENANT NAME --app-id APP_ID` to explicitly
associate a flat credential, or `tenant scanners set` with private TOML input for
separate keys. Reports retain management-visible metadata for unbound applications
and mark scanner detail incomplete. Direct scans and result/report lookups accept
`--scanner NAME`. This is an explicit configuration-format upgrade with private
rollback snapshots; see [named scanner setup](docs-site/docs/commands/tenant.md).
The unpublished 8.2 development tree's local history commands and automatic retention
were removed when history moved to the optional service; the frozen 8.1 RC4 packages
never included them. See [runtime history](docs-site/docs/commands/runtime.md#runtime-history-optional-service-in-development).

## Install

The command is `airs`. Install it through **one** channel; each one also provides the
names `airs-cli` and `airs-cli-rs` (except npm, which provides `airs` and `airs-cli`), the
same program printing the same output. The `airs-cli` name and the npm channel arrive
with 8.0.0.

**Homebrew** (macOS and Linux). The formula is in the public `cdot65/prisma-airs-cli`
tap ([github.com/cdot65/homebrew-prisma-airs-cli](https://github.com/cdot65/homebrew-prisma-airs-cli))
and builds `airs` from the published crates.io crate, so it needs Rust only while it
builds (Homebrew installs it as a build dependency). Homebrew loads a third-party tap's
formulae only after `brew trust`:

```sh
brew tap cdot65/prisma-airs-cli
brew trust cdot65/prisma-airs-cli
brew install airs-cli
brew upgrade airs-cli        # later releases
```

It installs `airs` and links `airs-cli` and `airs-cli-rs` to it. The tap carries stable
releases only. An install from the earlier Forgejo tap moves with `brew uninstall airs-cli`
and `brew untap cdot/tap`, then the lines above.

**npm** (from 8.0.0; macOS on Apple Silicon, Linux x64 and arm64; Node 18 or newer):

```sh
npm install -g @cdot65/prisma-airs-cli
```

It installs the `airs` and `airs-cli` commands: a small launcher that runs the prebuilt
binary from the matching platform package (an optional dependency; no install script,
no download). Intel macOS, Windows and other platforms get an error naming the other
ways to install. An npm 7.x install (the TypeScript CLI) is replaced by
`npm install -g @cdot65/prisma-airs-cli@latest`; then register your tenants with `airs tenant create`.

**From crates.io** (needs Rust 1.89 or newer). The crate is `airs-cli`; it installs
`airs`, `airs-cli` and `airs-cli-rs` (copies of the same program, so they cannot drift):

```sh
cargo install --locked airs-cli
```

**Prebuilt binaries** for Apple Silicon macOS (signed and notarised) and static Linux
x86_64 and arm64 are attached to each
[Forgejo release](https://git.cdot.io/cdot/prisma-airs-cli/releases) (it needs a
login), with a `SHA256SUMS` file. Each `airs-cli-<version>-<target>.tar.gz` holds
`airs`, the `airs-cli` and `airs-cli-rs` links to it, `README.md`, `LICENSE` and `NOTICE`.
The targets are `aarch64-apple-darwin`, `x86_64-unknown-linux-musl` and
`aarch64-unknown-linux-musl`:

```sh
shasum -a 256 -c SHA256SUMS --ignore-missing
tar -xzf airs-cli-<version>-<target>.tar.gz
mkdir -p ~/.local/bin
install -m 0755 airs ~/.local/bin/airs
ln -sf airs ~/.local/bin/airs-cli         # the other names, if you want them
ln -sf airs ~/.local/bin/airs-cli-rs
```

`~/.local/bin` must be on `PATH`, and ahead of any other `airs` (see below).

**From source:** every dependency, the SDK `scm-sdk` (`=0.2.4`) included, comes from
crates.io.

```sh
cargo build --release
```

This produces `target/release/airs` (and `target/release/airs-cli` and
`target/release/airs-cli-rs`, the same program under its other names). On Apple Silicon
it is a native arm64 executable. Copy it onto your `PATH`, or install it with
`cargo install --path .`.

```sh
airs --version     # prints the version, e.g. 8.2.0
airs --help
```

**One channel, and another `airs` on `PATH`.** Channels shadow each other by `PATH`
order. Homebrew's Node puts npm's global commands in `/opt/homebrew/bin`, where the
Homebrew formula links `airs` and `airs-cli` too, so an npm install and a Homebrew install
collide there; a 7.x TypeScript `airs-cli` or an unrelated npm package named `airs` may
also come first. Check with `type -a airs airs-cli airs-cli-rs` and `airs --version`
(this CLI prints a bare version such as `8.2.0`). `airs doctor` reports it: its first
check, `Executable`, warns when another `airs`, `airs-cli` or `airs-cli-rs` earlier on
`PATH` resolves to a different file than the running program (it never fails the run).
Keep one channel and uninstall the others.

## Upgrading from the TypeScript CLI (7.x)

The TypeScript CLI ends at 7.x. Install the Rust CLI through one channel above (npm users
can upgrade in place), then:

1. Register each tenant with `airs tenant create <name>`. `tenant import` was removed after
   8.2; version 8.2 of `airs` can still copy a TypeScript registry
   ([docs/MIGRATION.md](docs/MIGRATION.md)).
2. Update scripts: `aigateway` is `gateway` and `agentguard` is `supplychain` (no
   aliases); `completion`, `redteam judge` and `runtime dlp generate` are dropped;
   `--output markdown|csv|yaml` is a usage error (exit 2); argument-parse errors exit 2;
   failed scans fail closed (exit 1). Every difference is a DESIGN §12 row.
3. The `airs-cli` command keeps working and is now this program; help and hints name
   `airs`. The npm package no longer exports a JavaScript library.
4. Finish TypeScript bulk-scan jobs first and re-take backups with `airs`: their files
   are not read.

The upgrade guide on the docs site has the details:
<https://cdot65.github.io/prisma-airs-cli/>.

## Upgrading to 1.0

1. **The command is `airs`.** `airs-cli-rs` keeps working and prints the same output;
   switch scripts to `airs` at your pace. (In 1.x the TypeScript `airs-cli` was not
   affected; from 8.0 `airs-cli` is this program, see above.)
2. **Nothing to migrate.** Tenants (`~/.config/airs-cli/`), debug logs, bulk-scan jobs,
   backups and the fetched uv stay where they are, in the same formats: a 0.x bulk-scan
   job resumes under 1.0, and 0.x backups restore.
3. **Texts name `airs`.** Hints, remedies, usage lines and help examples say `airs`
   where 0.x said `airs-cli-rs`; scripts that match stderr text change accordingly.
   Data, exit codes and streams are unchanged.
4. **cargo:** `cargo install --locked airs-cli` installs `airs` and `airs-cli-rs` (two
   copies of one program); the 0.x `airs-cli-rs` is replaced.
5. **Tarballs** are now `airs-cli-<version>-<target>.tar.gz` with `airs` and the link.
   Install `airs` and replace a 0.x copy of `airs-cli-rs` with the link
   (`ln -sf airs ~/.local/bin/airs-cli-rs`); otherwise the old 0.x binary keeps
   answering to `airs-cli-rs`.
6. **Homebrew** is new. Remove a hand-installed `airs` or `airs-cli-rs` from
   Homebrew's `bin` first, or `brew link` refuses to overwrite it.
7. **Another `airs` on `PATH`** wins by `PATH` order: check with `type -a airs` and
   `airs --version` (it prints `1.x.y`).
8. **scm-sdk 0.2.3** (the red team management API 0.86.0): targets with a Vertex AI
   or `client_session` config are newly read (D329) and backed up, unless the service
   masks a Vertex AI `project_id`, `location` or `engine_id` (a failed row naming them
   and the remedy);
   `VERTEX_AI`, `N8N` and `REALTIME` targets, which 0.x read but could not back up or
   send, are now backed up and sent (D328, D329); a target's `audio_curl` is
   `[REDACTED]` like `curl` (D327); the four 0.86.0 fields (`audio_request_json`,
   `audio_curl`, `basic_username`, `basic_password`) print after the declared fields,
   no longer sorted among the unknown keys (D35); requests carry
   `PAN-AIRS/0.2.3-rust-sdk`. No other output changes.

## Usage

Commands use the selected tenant of the `airs` store. `--tenant <name>` selects
another registered tenant for one invocation and leaves the selection unchanged (it is
refused by the commands that change tenants, which take the name as an argument).

### Tenants

```sh
airs tenant create development           # prompts for the TSG ID, client ID and (hidden) client secret
printf '%s\n' "$SECRET" | airs tenant create ci --tsg-id 1234567890 --client-id ci-client --client-secret-stdin
airs tenant switch development           # select it for later commands
airs tenant set development defaultOutput json
airs tenant set development airsApiKey   # secrets: a hidden prompt ...
printf '%s\n' "$KEY" | airs tenant set development airsApiKey --stdin   # ... or stdin
airs tenant set development mgmtTsgId 2345678901   # changes the TSG and its pin
airs tenant unset development defaultOutput        # credentials cannot be cleared
airs tenant delete development           # asks first; --force skips the question
airs tenant list                         # every tenant as a table, no credentials read (alias: ls)
airs tenant path                         # tenant file path of the selected tenant
airs tenant read                         # effective config as a Key │ Value table, secrets [REDACTED]
airs tenant get development mgmtTsgId    # one setting (pretty by default)
airs tenant get development mgmtTsgId --output json
airs --tenant staging tenant read
```

- **Where tenants live.** `config.toml` (the selection and each tenant's pinned TSG)
  and `tenants/<name>.toml` (that tenant's settings, secrets included) in
  `$AIRS_CLI_CONFIG_DIR`, else `$XDG_CONFIG_HOME/airs-cli`, else `~/.config/airs-cli`.
  Files are mode 0600 and directories 0700. The setting keys are the TypeScript ones,
  with `aiGw*` renamed `gateway*` and `agentGuard*` renamed `supplychain*`; `dataDir`
  and the `typesafe*` keys are gone.
- **Values are checked when you enter them.** Numbers are decimal integers in range,
  endpoints must be https (http only for loopback hosts) without credentials, a query
  or a fragment, `mgmtTsgId` is 1–32 digits, and `defaultOutput` is `pretty`, `table`
  or `json`. An empty value is refused; use `tenant unset` to remove a setting. A
  prompt asks again and says why.
- **Secrets never go on the command line.** `tenant set <name> <secret key> <value>` is
  refused; omit the value for a hidden prompt, or pipe it with `--stdin`. Prompts need
  stdin and stderr to be terminals; in scripts use the flags and `--stdin`.
- **Deleting** removes the tenant's file and clears the selection if it was selected.
  Without a terminal it needs `--force`.
- **Outcome lines** (`✓ …`, `ℹ …`) go to stderr and `--quiet` hides them; stdout stays
  empty.
- **Concurrent changes are safe.** Every change takes one lock (`config.lock`; a
  second change waits up to 5 s and then names the process holding it) and replaces
  files atomically, so a reader never sees a half-written file and a crash never leaves
  a stale lock.

### Doctor

```sh
airs doctor
airs doctor --output json | jq '.[] | select(.status != "pass")'
airs doctor --output table                         # Name │ Status │ Detail │ Hint
airs --tenant staging doctor
```

The doctor runs eight checks: tenant, config file, environment, scanner credentials,
management credentials, scanner API, management OAuth and gateway API. It exits 1 when any
check fails. The gateway check lists the data-plane workspaces once with the management OAuth
token (one attempt; a 403 or 429 warns, D250). The TypeScript doctor's Node.js and Typesafe
checks are not ported (D19).

### Profiles and topics

```sh
airs runtime profiles list                         # latest revision of each profile (alias: ls)
airs runtime profiles list --limit 20 --offset 20
airs runtime profiles list --all                   # walk every page, up to --max (default 10000)
airs runtime profiles list --all --max 0           # no item cap
airs runtime profiles list --all-versions
airs runtime profiles get prod-guard               # by name (highest revision) or UUID
airs runtime profiles get prod-guard --revision 3
airs runtime profiles get prod-guard --all-versions

airs runtime topics list                           # alias: ls; same paging flags
airs runtime topics list --all
airs runtime topics get my-topic
airs runtime topics get my-topic --revision 2
```

Without `--output`, these commands use the tenant's `defaultOutput` (`pretty`, `table`
or `json`), else `pretty`: a banner, then one entry per profile or topic (lists) or a
key/value block (gets). `--output table` prints a box-drawing table (`ID │ Name │ …` for
lists, `Key │ Value` for gets), and `--output json` a JSON array for lists and a JSON
object for gets. With `--all-versions`, a get prints the list layout of every matching
revision. A status line such as `Showing 20 (more available)` goes to stderr in every
format. A `defaultOutput` of `markdown`, `csv` or `yaml` makes the tenant config invalid
(D51).

### Writing profiles and topics

```sh
airs runtime topics create --name "Financial Advice" --description "Requests for investment advice" \
  --examples "Should I buy TSLA stock?" "How should I invest my savings?"   # creates, or updates the existing name
airs runtime topics update <topicId> --config topic.json
airs runtime topics delete <topicId>                      # asks first (alias: rm)
airs runtime topics delete <topicId> --force --updated-by me@example.com

airs runtime profiles create --name prod-guard --prompt-injection block --toxic-content block
airs runtime profiles create --config profile.json        # the file is the whole body; its profile_name names the profile
airs runtime profiles update prod-guard --url-action alert             # merges into the latest revision
airs runtime profiles update prod-guard --no-active
airs runtime profiles delete prod-guard                   # asks first; by name or UUID (alias: rm)
airs runtime profiles delete prod-guard --force --updated-by me@example.com

airs runtime topics apply --profile prod-guard --name "Financial Advice"            # --intent block (default) or allow
airs runtime topics revert --profile prod-guard --name "Financial Advice"           # asks first; --force in scripts
```

- **Every check runs before the tenant is read.** Topic limits (UTF-8 bytes: name 100,
  description 250, 2–5 examples of 250, 1,000 together), UUIDs, `--updated-by` with
  `--force`, a missing policy flag, `--max-inline-latency` (a whole number of seconds),
  `--dlp-profiles`/`--[no-]mask-data-inline` without `--dlp-action` on a create, and the
  `--direction`/`--ai-profile-index` values are usage errors (exit 2). `--config` files are
  read and checked against the request schema before the tenant too (at most 1 MiB, a
  UTF-8 JSON object); `--config` cannot be combined with any other write flag or
  selector.
- **A write is sent once.** It is never re-sent after a timeout or a 5xx, so a create
  can never be duplicated; only a 401 is replayed once with a fresh token. When the
  service may have applied a failed write, the error says
  `Outcome unknown: the topic may have been created.`; a reply that could not be read
  after a success says so too. Exit 1 in both cases.
- **Updates change only what you ask for.** `profiles update` keeps `active` unless
  `--active`/`--no-active` is given, merges detectors by name (keeping their severities
  and topic lists), keeps the DLP profiles when only `--dlp-action` changes, writes an
  explicit `false` for the `--no-…` flags, and refuses an update with nothing to change.
  `topics create` and `profiles update` act on the highest revision of a name.
- **Confirmations** (`delete` without `--force`) need stdin and stderr to be
  terminals; the prompt is on stderr and only `y`/`yes` confirms. In scripts pass
  `--force` (with `--updated-by <email>`, which selects the force endpoint).
- **Applying and reverting topics.** `topics apply` reads the topics, then the
  profile's highest revision, and writes that profile once with the topic's latest
  revision under its intent; other topics' entries stay exactly as they were, and
  nothing is written when the topic is already there. The service has no conditional
  write, so a change someone else makes to the profile between that read and the write
  is overwritten (the window is one round trip). `topics revert` removes the topic from
  the profile (the `topic-guardrails` entry goes with its last topic), then deletes the
  topic with a plain delete; a profile that does not reference the topic is refused
  (exit 2) and nothing is deleted. A topic that is still referenced (by another profile, or by
  an older revision of this one, since every profile update creates a new revision) is
  kept with a warning and the command still exits 0; run `profiles cleanup`, then
  `topics delete`. It asks after looking the topic up.
- **Output.** Outcome lines (`✓ Topic created: …`, `Profile created: …`) go to stderr
  (`--quiet` hides them); stdout carries only data: the topic record or the profile
  detail, in `pretty`, `table` or `json` (the tenant's `defaultOutput` is used by
  `profiles create` and `update` only, as in the TypeScript CLI 7.3.0). The deletes print
  no data. The TypeScript CLI's hidden `--format` still works on `topics create`,
  `apply`, `revert` and `eval`, with its deprecation warning.

### Directional profiles (8.0)

A directional profile keeps its protections per direction (`prompt`, `response`,
`tool-call`, `tool-response`) under `content-type-configurations`, and its shared settings
(latency, storage masking, conversation inspection) in `model-configuration`. Legacy
profiles need no migration. This follows the TypeScript CLI 7.3.0 and `scm-sdk` 0.2.4. A
flag create writes the three directions it does not fill as empty objects, because the
service requires all four direction keys (the TypeScript CLI writes only one, which the
service refuses, D335).

```sh
airs runtime profiles create --name Directional --direction response \
  --toxic-content 'high:block, moderate:allow' --no-enable-full-conversation-inspection
airs runtime profiles update Directional --direction response --toxic-content 'high:block, moderate:block' --no-mask-data-inline
airs runtime profiles update Directional --max-inline-latency 8 --no-mask-data-in-storage   # shared: no direction
airs runtime profiles update Directional --config directional.json   # complete replacement (adds a direction)
airs runtime topics apply --profile Directional --name Restricted --direction prompt
airs runtime topics revert --profile Directional --name Restricted --direction prompt --force
```

- Protection edits on a directional profile need `--direction`, even with one direction,
  and the direction must exist (add one with a complete `--config`). A legacy profile
  refuses `--direction`; convert it with `--config`. Several AI entries need
  `--ai-profile-index <n>` (zero-based), shared settings included.
- These refusals come after the profile is read and write nothing (exit 2).
- `topics revert --direction` refuses while another direction, AI entry or a retained
  legacy block still references the topic.
- `profiles get` shows `Protection locations`, and `DLP tenant` (json `dlpTenantId`) only when
  the record carries one; in the live run no `get` or `list` returned it. Settings you do
  not pass are never written; unknown directions are kept; a `--config`
  file's new fields are checked before the tenant (a string `dlp_tenant_id`, never `null`).
  `runtime report` compares current protections by detector and direction, and profile backup and
  restore carry every direction.

#### Known limitations of directional profiles

Directional profiles are **supported, for single-entry profiles**: a live run (2026-10-22)
accepted every direction for creates, edits, topics, backup and restore.

- **Not verified live:** profiles with several AI entries (`--ai-profile-index`), DLP blocks
  inside a direction (`--dlp-action`, `--dlp-profiles`), and the Signal Brief's current protection comparison. The offline tests cover them.
- **Service rules are not checked before sending, except one.** Every directional entry needs
  all four direction keys (`prompt`, `response`, `tool-call`, `tool-response`; a flag create
  writes the ones it does not fill as `{}`, and a `--config` body must list all four); the
  `response` direction refuses `prompt-injection`; and `toxic-content` takes only `block` or
  `allow` for each level. A write that breaks one fails with HTTP 400 (exit 1), and no
  response body is shown. The exception (D336): a directional flag create with no protection
  flag (none, or shared settings only) is refused before anything is sent (exit 2), because
  the service refuses a policy with no protection in any direction. A `--config` body with
  four empty directions is still sent, and refused.
- **`dlp_tenant_id` on writes.** These writes do not send `dlp_tenant_id`: a flag
  `profiles update`, `topics apply` and `revert`, `profiles restore` (create and overwrite),
  and `profiles update --config` when the file leaves the key out. The service's answer then
  reports `dlpTenantId: "none"`; the effect on the profile's stored DLP tenant binding is
  unknown. No `get` or `list` returns the field, so the CLI cannot carry it over. This
  matches the TypeScript CLI 7.3.0 and affects legacy profiles too. A `--config` file that
  holds `dlp_tenant_id` sends it.

The full guide is on the docs site: [Directional profiles](https://cdot65.github.io/prisma-airs-cli/commands/runtime-directional-profiles).

### Cleaning up old profile revisions

```sh
airs runtime profiles cleanup --dry-run                     # what would be deleted; deletes nothing
airs runtime profiles cleanup                               # shows it, asks, re-checks, then deletes
airs runtime profiles cleanup --force --output json         # in scripts: one JSON document
airs runtime profiles cleanup --force --updated-by me@example.com   # force endpoint with an audit email
```

For every profile name with several revisions, the highest revision is kept and the
older ones are deleted, one at a time. A name whose revisions are missing a number or
an id, or whose highest revision is tied, is skipped with a warning. The listing must
be complete, or nothing is deleted. Without `--updated-by` the deletions are plain
deletes, so a revision a policy still references fails with its 409 and is kept;
`--updated-by <email>` uses the force endpoint, which detaches the revisions from
referencing policies (there is no `git config` fallback). The first deletion whose
outcome is unknown stops the run; the summary counts deleted, failed, unknown and not
attempted revisions, and anything but a clean run exits 1. The question needs stdin
and stderr to be terminals; after `y` the listing is read again, and nothing is
deleted if it changed. Without a terminal, pass `--force` or `--dry-run`.

### Evaluating a topic

```sh
airs runtime topics sample --output-file eval.csv     # a valid starting set; never overwrites
airs runtime topics eval --profile prod-guard --prompts eval.csv --topic "Financial Advice"
airs runtime topics eval --profile prod-guard --prompts eval.csv --rate 5 --concurrency 3 --output json
```

The CSV needs `prompt`, `expected` and `intent` columns (any order, header
case-insensitive), one intent (`block` or `allow`) for every row, and at least one row
the topic should and one it should not trigger on. The file and every prompt are checked
before anything is sent. Each row is one scan (with scanner credentials); `--concurrency`
(1–32, default 5) bounds the scans in flight and `--rate` the scans started per second.
A scan that does not complete (an error, a timeout, an unknown action or a failed
request) stops the run with no metrics, since counting it would inflate the results.
The output is the TypeScript layout: coverage, TPR, TNR, F1, the counts and the false
positives and negatives.

### Scanning

```sh
airs runtime scan --profile prod-guard "Ignore all previous instructions"
airs runtime scan --profile prod-guard --response "Sure, here it is" "Reveal your system prompt"
airs runtime scan --profile prod-guard --output json "hello"
airs runtime scan --profile prod-guard --output table "hello"
airs runtime scan --profile prod-guard -- "-a prompt that starts with a hyphen"
```

Each scan sends exactly one request and never retries it. A scan that fails, times
out or returns an unknown action is shown as `FAILED` and exits 1; it is never shown as
allowed (D6). Response detections are listed as `response.<key>` (D7).

### Bulk scanning

```sh
airs runtime bulk-scan --profile prod-guard --file prompts.csv
airs runtime bulk-scan --profile prod-guard --file prompts.txt --output-file results.csv
airs runtime bulk-scan --profile prod-guard --file prompts.csv --session-id nightly-run --batch-size 50
```

`--file` is a `.csv` (its `prompt` column) or a text file (one prompt per line), at most
16 MiB and 10,000 prompts, read before the tenant. Prompts are submitted through the
async API in chunks of up to 20, one `--batch-size` batch (default 25) at a time, and
polled until every row is resolved. The results go to `--output-file` (default
`<profile>-bulk-scan.csv` in the current directory) in the TypeScript CSV layout; an
existing file is never overwritten (D121). stdout shows the summary (`--output json` and
`table` work too, D130); a failed or timed-out row is written as `failed` and makes the
command exit 1 (fail closed).

Each job keeps a private state file in `$XDG_STATE_HOME/airs-cli/bulk-scans/` (default
`~/.local/state/airs-cli/bulk-scans/`; 0700 directory, 0600 files) that is written durably
before every submission and before every poll (D118), with a per-job lock (D119). A
submission that may have reached AIRS (a server error, a dropped connection, Ctrl-C while it
is in flight) is recorded as ambiguous and never submitted again; a refused or rate-limited
one stays pending. Ctrl-C saves the job and names its state file (D129). State files keep
the prompts; delete a finished job's files when you no longer need them.

```sh
airs runtime resume-poll ~/.local/state/airs-cli/bulk-scans/<job>.bulk-scan.json
airs runtime resume-poll <state> --output-file results-2.csv
```

`resume-poll` continues a stopped job: it polls the receipts already accepted, submits
only the prompts whose submission was refused or never sent, and rewrites the job's CSV
with every result (the same bytes again on a finished job, with no request). It resumes
only a job in its state directory, under the tenant and scan endpoint that started it
(D127); a TypeScript `airs-cli` state is refused (D118). When a submission's outcome is
ambiguous, it first recovers the results of the accepted receipts, then stops and names
the lowest such prompt: nothing is ever resubmitted automatically, so review the session's
scans in the AIRS dashboard. `--output-file` moves the job's output to a new file; an
existing other file is never overwritten (D121). A second run on the same job is refused
at once, naming the process that holds it (D119).

### Red Team (reads)

```sh
airs redteam list --status COMPLETED --limit 5
airs redteam status <jobId>
airs redteam categories --output json
airs redteam targets list --all --output json
airs redteam targets get <uuid>
airs redteam prompt-sets download <uuid> --output-file template.csv
airs redteam network-broker channels list --status ONLINE OFFLINE
```

The red team commands use the selected tenant's `redTeamDataEndpoint`,
`redTeamMgmtEndpoint` and `redTeamNetworkBrokerEndpoint` (else the AIRS defaults) with its
management credentials. Every read prints `pretty`, `table` and `json`; the lists and gets
honour the tenant's `defaultOutput`. Ids, numbers and filters are checked before the
tenant is read (a bad UUID is a usage error, exit 2). From 0.9.1 (scm-sdk 0.2.2) the
values the red team API contract refuses are usage errors too: `--limit` of `list`,
`targets error-logs` and `report <jobId> --attacks` is at most 100; `list --status`,
`--type` and `--target` and `report --attacks --severity` take the API's values (for
example `PARTIALLY_COMPLETE`, `CLARA`, a target UUID, `HIGH`);
`targets error-logs --offset` and `--search` are refused, because the API pages error logs
by `--limit` only; and `scan --depth` and `--breadth` are at most 20 (D300–D305). The
`report --attacks` checks apply whatever the job type, so a CUSTOM or DYNAMIC job's unused
`--severity` (and a DYNAMIC job's `--limit`) must be valid too. From 0.9.1 answers are no
longer checked for string formats, so where the TypeScript CLI and 0.9.0 refuse an answer
on a format (an adapter whose `created_by_user_id` is not a UUID, for one) airs-cli-rs
reads and prints it (D306); `adapter update` names a stored channel that is not a UUID and
asks for `--channel` (D307). Red team scan, attack and error-log answers read as before:
the new SDK keeps accepting an id that is not a UUID, a date that is not RFC 3339 or a new
status value, which scm-sdk 0.2.0 would have refused (G25 closed). From 1.0 (scm-sdk
0.2.3, the red team management API 0.86.0) `targets create`, `update`, `probe` and
`restore` send the new values the TypeScript CLI refuses: `connection_type` `VERTEX_AI` and
`N8N`, `response_mode` `REALTIME`, the `client_session` multi-turn config and a Vertex AI
connection config (`project_id`, `location`, `engine_id`, `customer_sa_email`; a Vertex AI
value outside the API's pattern is refused before anything is sent) (D328). A target with a
Vertex AI or `client_session` config is newly read, printed (redacted) and backed up, where
the TypeScript CLI and 0.11 refuse the answer (a Vertex AI target whose answer masks its
`project_id`, `location` or `engine_id` cannot be sent back, so the backup lists it as failed,
naming the masked fields and telling you to recreate the target or set them by hand, and
exits 1; a mask only in `customer_sa_email` is stored, marked as
withheld); a `VERTEX_AI`, `N8N` or `REALTIME` target
without either config, which both already read and print, is now also backed up, where 0.11
recorded the `its settings do not match the target create schema` failed row (TypeScript
stores it, D151) (D329).
Target, adapter and error-log documents never show a credential: auth settings (the basic
username and password included), secret headers, `curl` and the audio template
`audio_curl` (D327), provider keys and the Vertex AI settings, secret-named keys and
`SECRET` adapter variables are `[REDACTED]` in every format (D140). Lists are complete or an error: `prompt-sets list` reads every page and an
`--all` walk that meets an empty page before the reported total fails (D147).
`prompt-sets download` never overwrites: an existing path is refused before any request,
the template is written at mode 0600, and `--output-file -` prints it (D142). See DESIGN
§15 for the whole contract.

### Red Team (writes and scans)

```sh
airs redteam scan --target <uuid> --name nightly                 # STATIC, all categories, waits
airs redteam scan --target <uuid> --name probe --type DYNAMIC --goals goals.json --no-wait
airs redteam abort <jobId> --force
airs redteam targets init rest                                   # rest-target.json, mode 0600
airs redteam targets create --config rest-target.json --validate
airs redteam adapter create --name a --script-file adapter.py --prompt Hello --variables-file vars.json
airs redteam prompt-sets upload <uuid> prompts.csv
```

Every red team write is sent once: a 5xx, a timeout or a lost connection is never re-sent;
the error says the outcome is unknown, and for a scan start, an abort, the EULA and an upload
it names the command that shows what happened (D133). Only a 401 is replayed, once, with a
fresh token. `scan` waits with progress on stderr for at most `--wait-timeout` minutes
(default 1440); a scan that is still running when the wait ends, fails or is interrupted is
reported as started and never aborted; `ABORTED` exits 1 and `PARTIALLY_COMPLETE` exits 0
with a warning (D135, D136). `abort`, `eula accept` and the deletes ask on a terminal; scripts
pass `--force` (without a terminal and without `--force` they are usage errors, D137, D138).
`devices delete` refuses a `--serial-numbers` list that names no serial number, and
`eula accept` an empty EULA (D158, D159).
Outcome lines and hints go to stderr; stdout holds the data in the chosen format. The
registry token is shown only with `--output json` (D139); credentials in answers and the
validate-auth token preview are `[REDACTED]` (D140). Input files (`--config`, `--goals`,
`--script-file`, `--variables-file`, the upload CSV) are read before the tenant with size caps;
`SECRET` adapter variables are refused on the command line, so put them in a
`--variables-file` (D143, D154). `targets init` never overwrites and writes mode 0600 (D150).

### Red Team (reports, backups and restores)

```sh
airs redteam report <jobId> --attacks --severity HIGH            # one scan's report
airs redteam report <jobId> --output json
airs redteam report                                               # airs-redteam-report-<time>-<id>.html, 0600
airs redteam report --strict --output-file - > daily.html
airs redteam targets backup --output-dir ./backups                # one 0600 file with every target
airs redteam targets restore --file backups/airs-redteam-targets-….json --dry-run
airs redteam targets restore --file backups/airs-redteam-targets-….json --overwrite
```

A scan job report prints the TypeScript layout, or `--output json`/`table` (D144). The
environment report reads seven sources (aggregates only, no names, ids or settings) and is
written as self-contained HTML only: `--output` is refused, and so is `--debug` (D155, D156);
it never overwrites a file and is mode 0600. `targets backup` writes one airs-cli-rs file per
run into an existing directory (default the current directory), mode 0600, never
overwritten; it holds every credential the service returns, so keep it private (D151). It
stores only the settings the service sent. A target whose credentials the service withholds
(masks) is stored as returned, marked `credentialsWithheld` and listed with `⚠`; restore
refuses it, so recreate it or set its credentials by hand (D157). `targets restore` reads only such a file (a
TypeScript backup is refused with a message), checks that it comes from the selected tenant's
TSG (`--allow-other-tenant` overrides), refuses duplicate or ambiguous names and withheld
credentials before any write, offers `--dry-run`, asks before `--overwrite` on a terminal
(`--force` for scripts), sends each write once and stops at the first unknown outcome (D152).

### DLP

```sh
airs runtime dlp patterns list
airs runtime dlp patterns list --include-predefined --limit 20 --offset 20
airs runtime dlp profiles get <id> --output json
airs runtime dlp dictionaries list --keywords --output json
airs runtime dlp filtering-profiles list --all
airs runtime dlp patterns create --name "Employee ID" --regex 'EMP-[0-9]{6}' --confidence-levels high
airs runtime dlp patterns patch <id> --set name="Employee ID" type=custom 'detection_config={"technique":"regex"}'
airs runtime dlp profiles create --name "HR data" --pattern-id <pattern-id>
airs runtime dlp dictionaries create --name Codes --category Confidential --region "United States" --file ./codes.txt
airs runtime dlp patterns delete <id>            # asks on a terminal; --force for scripts
airs runtime dlp backup --output-file ./dlp-backup.json
airs runtime dlp restore ./dlp-backup.json --name-prefix migrated- --dry-run
airs runtime dlp restore ./dlp-backup.json --name-prefix migrated- --expect-tsg <tsg> --force
```

The DLP commands use the selected tenant's `dlpEndpoint` (else
`https://api.dlp.paloaltonetworks.com`) with its management credentials. Every read prints
`pretty`, `table` and `json` and honours the tenant's `defaultOutput`. Pattern, profile and
dictionary listings show the tenant's own (custom) records by default, read from every page;
`--include-predefined` shows the service's pages as they are. Ids, numbers and `--sort` are
checked before the tenant is read (exit 2). JSON is the full record in camelCase, with the
keys of your tags and exclusion lists kept as they are (D171); keywords are fetched only with
`--keywords`. DLP records hold detection content (regexes, keywords): it is printed only by the
json and table reads and never in an error or the `--debug` log.

The writes build their body from flags, or take it whole from `--body <json>`, `--body -`
(piped stdin) or `--body-file` (never both; patches take `--set key=value`/`--clear key` or
`--body-file`). Every argument and body is checked before the tenant is read. Before a
replace, patch or delete the record is read: predefined (PANW-shipped), parent-managed and
unfamiliar-type records are refused, and nothing is sent. Deletes and replaces ask for
confirmation on a terminal and need `--force` otherwise; a replace names the fields it would
drop. Each write is sent once (a 401 is retried once with a fresh token): when its outcome is
unknown the error says so and, for a create, how to check before creating it again.
`patterns replace` keeps the pattern's type and technique unless you give them. The API has no
data-profile DELETE, so `profiles delete` explains that and sends nothing (exit 2). Outcome
lines go to stderr; `--output json` and `table` print the acknowledgement on stdout.

`runtime dlp backup` writes one airs-cli-rs file (mode 0600, never overwriting anything; the
path is checked before the tenant is read) holding your custom dictionaries with their
keywords, patterns and profiles, with every predefined record a profile references as a
reference only. It records what it could not store (`skipped`, `failed`, `complete`); a
dictionary whose keywords cannot be read is a failed row (exit 1). The file holds detection
content in clear, so keep it private. `runtime dlp restore <file>` reads only airs-cli-rs
backups (a TypeScript backup is refused), refuses a backup from another TSG unless you pass
`--allow-other-tenant`, plans every row against the destination before any write (predefined
references resolved by id or name, `--pattern-map` for the rest, `--name-prefix` for copies),
offers `--dry-run`, asks on a terminal (`--force --expect-tsg <tsg>` for scripts), only ever
creates (never updates or deletes), sends each create once and stops at the first failure,
naming a resource that may have been created. `--on-conflict reconcile` gives an
archived-name collision a deterministic suffix, so a re-run verifies that copy instead of
creating another. See DESIGN §16 for the whole contract.

### Model Security (reads)

```sh
airs model-security groups list
airs model-security groups get <uuid> --output json
airs model-security rules list --source-type LOCAL --all
airs model-security rule-instances list <groupUuid> --state BLOCKING
airs model-security scans list --eval-outcome BLOCKED --start 2026-01-01T00:00:00Z
airs model-security scans list --scan-origin MODEL_SECURITY_SDK --all --output json
airs model-security scans evaluations <scanUuid> --output table
airs model-security models versions <modelUuid> --all
airs model-security labels values env
```

The model-security commands use the selected tenant's `modelSecMgmtEndpoint` (groups, rule
instances, rules) and `modelSecDataEndpoint` (scans, models, labels), else
`https://api.apps.paloaltonetworks.com/aims/mgmt` and `…/aims/data`, with its management
credentials. Every read prints `pretty`, `table` and `json` (the scan sub-reads and the label
lists too, which the TypeScript CLI prints in pretty only, D189); the group, rule,
rule-instance, scan and model reads honour the tenant's `defaultOutput`. UUIDs, the time
window, numbers and filters are checked before the tenant is read (exit 2, D184). Nothing the
service did not send is printed: a scan summary without counts shows no `Rules` line, and its
json counts are `null` (D180). Model URIs are printed with their credentials, query and
fragment `[withheld]` (D191). `--all` walks every page and fails rather than print a partial
list; every list honours `--offset`, `--all` and `--max` (D183); a page that does not show
everything says so on stderr. `scans list --scan-origin` filters exactly over every page
(the service has no such filter, D181). See DESIGN §17 for the whole contract.

### Model Security (writes)

```sh
airs model-security groups create --config group.json
airs model-security groups update <uuid> --description ''
airs model-security groups delete <uuid>            # asks; --force for scripts
airs model-security rule-instances update <groupUuid> <instanceUuid> --config state.json
airs model-security scans create --config scan.json --output json
airs model-security labels add <scanUuid> --labels '[{"key":"env","value":"prod"}]'
airs model-security labels set <scanUuid> --labels '[{"key":"env","value":"prod"}]' --force
airs model-security labels rm <scanUuid> --keys env,team --force
```

Every write is sent once: after a timeout, a 5xx or a dropped connection the CLI says the
outcome is unknown instead of re-sending (a 401 is retried once with a fresh token), so a
billed `scans create` is never sent twice (D179). `groups delete`, `labels set` (which
replaces every label and names the ones it drops) and `labels delete` ask on a terminal and
need `--force` elsewhere (D185). `groups delete` re-reads the group and reports `deleted` only
on a 404 or a tombstone; a group the service still shows gets a warning (D186). A group config
takes `name`, `source_type`, `description` and `rule_configurations`, a rule-instance config
`state`, `field_values` and `security_group_uuid`; other keys, a file that changes nothing and a
`groups update` without a flag are refused before the tenant is read, and no file or label
content is ever echoed in a message (D187, D188). `--description ''` clears a description. The
written record prints in the chosen `--output` on stdout, the `✓` line on stderr (D189, D190).

### Model Security PyPI access and `install`

```sh
airs model-security pypi-auth                         # the index URL, credential withheld
airs model-security pypi-auth --url-file pypi.url     # the full URL into a new 0600 file
airs model-security install --dry-run                 # the uv commands, credential withheld
airs model-security install --extras aws --dir ./ms   # uv init + uv add
airs model-security install --fetch-uv                # use a pinned, verified uv 0.12.17
```

The PyPI index URL carries a short-lived access token. `pypi-auth` prints it with the credential
`[withheld]` in every format; the only way to get the full URL is `--url-file <path>`, a new file
created with mode 0600 that is never overwritten (D194). `install` uses uv only (0.12.0 or
later on `PATH`; TypeScript's python/pip fallback is gone, D195): it runs `uv init
--no-workspace -- <dir>` and `uv add --project <dir> --index airs-model-security=<index> --
'model-security-client[<extras>]'`, handing the credential to `uv add` only through
`UV_INDEX_AIRS_MODEL_SECURITY_USERNAME`/`_PASSWORD` in its environment, never in its arguments,
and checks afterwards that `pyproject.toml` and `uv.lock` do not hold it (D196). `--dir` must not
exist yet and must not lie inside a uv workspace (D198). uv's output is scrubbed and goes to
stderr; stdout stays empty (D199). Ctrl-C is passed on to uv, which the CLI waits for (D200).
`--dry-run` prints the commands with the credential withheld, quoted for a POSIX shell (D197).
`--fetch-uv` downloads uv 0.12.17 for your platform into `$XDG_DATA_HOME/airs-cli/tools`
(else `~/.local/share/airs-cli/tools`), verifies its pinned SHA-256 before extracting, and reuses
a copy that verifies. Later uv commands in the project need the credential again: set the two
variables from the URL `pypi-auth --url-file` saves.

### Runtime extras (reads)

```sh
airs runtime api-keys list --all
airs runtime customer-apps list --output json
airs runtime customer-apps get my-app
airs runtime customer-apps consumption chatbot --time-interval 7
airs runtime customer-apps consumption --output json   # every dashboard app, one document
airs runtime deployment-profiles list --unactivated
airs runtime dashboard applications --interval 7 --unit days --output table
airs runtime sessions list --interval 1 --unit day --all --output json
airs runtime sessions transaction --session-id <id> --app-id <id> --app-name <name> --scan-id <id> --scan-sub-req-id 0
airs runtime sessions scan-content --scan-id <id> --scan-sub-req-id 0 --output-file scan.json
```

API keys, customer apps and deployment profiles use the tenant's `mgmtEndpoint`; the dashboard
and sessions commands, and `consumption`, use its `mgmtDashboardEndpoint` with the same OAuth
identity. An API key value that a read carries is never printed (D205); auth codes are printed on
stdout only. `consumption` looks the app up over the `--time-interval` window and refuses a name
two applications share (D211); without a name it prints one JSON document and exits 1 when an app
failed (D210), each app's reads having their own 60 s request budget however many apps the tenant
has. The dashboard and sessions commands print the response as JSON text in `pretty`
and `json` (control characters escaped) and also support `table` (D212); any failure after the
client is built is TypeScript's one `Dashboard operation failed` line. `sessions list --all` keeps
TypeScript's page checks and exits 1 after printing a partial list at its item or page cap.
`sessions scan-content` prints the stored prompt and response only with `--show-content`, or
writes them into a new 0600 `--output-file`, checked before any request (D225). `scan-logs query` discovers sessions and resolves scan results using the SDK, with explicit coverage and incomplete-read exit 1. Direct `results --scan-id` and `reports --report-id` use scanner credentials only. The old stub endpoint is not used.

### Supplychain

```sh
airs supplychain scans list                                  # one page of 10 scans
airs supplychain scans ls --all --max 0 --output json        # every scan
airs supplychain scans list --start 2026-10-01T00:00:00Z --end 2026-10-20T00:00:00Z
airs supplychain scans vulnerabilities <scanUuid>            # finding metadata only
airs supplychain scans vulnerabilities <scanUuid> --include-content --output json > findings.json
airs supplychain stats
airs supplychain rules list --all
airs supplychain report                                      # new 0600 HTML file in the working directory
airs supplychain report --strict --max-pages 20 --output-file - > supplychain.html
```

The supplychain commands (TypeScript `agentguard`) use the tenant's `supplychainDataEndpoint`
and `supplychainMgmtEndpoint` (else the SDK defaults) with the management OAuth identity and the
`x-tsg-id` header. Every request is a read, sent once (a 401 or 403 is re-read once with a fresh
token). `scans list` prints nine fields of each scan and never its name, git URL, fingerprint,
device metadata or error text. `scans vulnerabilities` prints four fields of each finding;
`--include-content` prints each finding whole (titles, file paths, code and remediation text,
which can contain secrets from the scanned artifacts) on stdout only, with a notice on stderr
(D321); it never reaches stderr or the debug log. The service cannot page findings: when it
reports more findings than it returned, the rows are printed and the command fails with exit 1
(D319). `stats` is always a `Field │ Value` table (D318). `pretty` is the table; the status line
says when a list is truncated and names the next `--offset` (D313). `--limit`, `--offset` and
`--max` take decimal digits only, `--start` and `--end` ISO-8601 times with a zone, and every
argument check runs before the tenant is read (exit 2, D314, D315). A failure prints one line
with at most the HTTP status and no server text; a missing credential or a bad endpoint keeps
its own message (D316). Answers are bounded at 32 MiB and 1,000,000 JSON values (D320).

The service sends `null` in scan fields the published API contract requires (for example a
scan's `fingerprint`), which makes the TypeScript CLI refuse the whole scan list. airs-cli-rs
reads the scan list itself (G71): a `null` field prints as `null` in JSON and an empty cell in
the tables, and every other row prints exactly as before (D324). A scan list it still cannot
read, such as a scan without an id, fails with `The AgentGuard answer does not match the API
contract; nothing was printed.` (exit 1, D325). The report counts such rows too: a missing
creation time excludes the row, a row marked as a batch or listing child scans is a parent
either way, and a row whose batch membership neither field decides is left out of the outcome
counts with a note. An empty answer to a scan list prints the request failure line, as
TypeScript does.

`supplychain report` writes an aggregate-only HTML report of the last 30 days: scan outcomes by
artifact type (parent and batch rows excluded), the server's 30-day skill statistics and the rule
catalog's default states, with an evidence table that marks every source `complete`, `partial` or
`unavailable`. No scan name, id, URL, fingerprint, device metadata, rule text or finding text is in
it. The default file is `airs-supplychain-report-<time>-<8 hex>.html` (D310), created new at 0600
and never overwriting; `--output-file -` writes to stdout. HTML is the only format (D155) and
`--debug` is refused (D156); `--max-pages` (1–100, default 40) bounds each paged source. Creation
times must carry a zone (a date alone is UTC midnight; D322). Exit 1 when no source could be read,
or with `--strict` when one is incomplete; the document is written either way. Any other failure
prints one line, `Supply chain report failed …` (D311).

### Gateway (reads)

```sh
airs gateway workspaces list                       # the data plane: workspaces you are scoped to
airs gateway workspaces list --all --output json   # admin plane, active and archived
airs gateway workspaces get ws-main-a-349e0e --plane admin
airs gateway scopes list
airs gateway scopes get ws_production_bx7qw0 --output json
airs gateway telemetry cost --workspace "Main A" --days 30   # a display name: needs the admin grant too
airs gateway telemetry requests --workspace ws-main-a-349e0e --start 2026-10-01T00:00:00Z --end 2026-10-02T00:00:00Z
airs gateway telemetry group-by model --workspace ws-main-a-349e0e --columns cost,total_tokens
airs gateway telemetry logs list --workspace ws-main-a-349e0e --page-size 20 --current-page 1
airs gateway configs list --workspace 16f7e90d-382a-4e78-b577-1b01eb5f8297
airs gateway providers get 2c1e5f0a-7d43-4b8e-9a51-0f2d3c4b5a69 --output json
airs gateway integrations providers                # the catalog slugs
airs gateway audit-logs list --days 1 --reveal-sensitive
airs gateway organisations auth-settings get       # the selected tenant's TSG
airs gateway admin-guardrails list --page-size 50 --current-page 0
```

The gateway commands use the tenant's `gatewayDataEndpoint`, `gatewayAdminEndpoint` and
`iamEndpoint` (else the SDK defaults) with the management OAuth identity and the `x-tsg-id`
header. Every telemetry command resolves `--workspace` (a slug, UUID or unique display name)
over the data plane, then the admin plane's active and archived lists, and refuses a reference
nothing matches before any telemetry request (D235). A slug or UUID the data plane lists needs
only the data-plane grant; a display name also reads the admin-plane lists (an exact slug there
wins over a name) and so needs the admin grant too, and a 403 on those lists says to use the
slug or UUID. `--start` and `--end` take ISO-8601 times
with `Z` or an offset and are sent in UTC whatever the host's time zone; `--days` with `--start`
is refused (D242). Every argument check runs before the tenant is read (exit 2, D232). `table`
prints a table and an empty json list prints `[]` (D231); acknowledgements, warnings and the 403
grant hint go to stderr (D230, D234), and a list the service says is incomplete is printed with a
warning (D248). Every gateway document goes through one set of rules in every format (D238,
D251; DESIGN §19.3): the SDK's secret paths for the resource; members whose names are credential
names in any case or separator (`clientSecret`, `API KEY`, `api.key`, `db_pwd`, `db_pass`,
`GITHUB_PAT`, `refreshTokens`, `tokenvalue`, `account_key`, `access_keys`, `basic_auth`,
`signature`, `passcode`, `otp`, a Vault `secret_id`, any name carrying a credential word such as
`adminpassword`: every name TypeScript masks but listed identifiers, references, token counts, key
names and lists of them (`metadataKeys`, `virtual_keys`), guardrail settings and `tokenizer`, a secret mapping's `secret_key`, and a name whose last
words describe a credential, such as `user_api_key_count`, `user_api_key_ttl`, `api_key_name`,
`token_type`, `token_expires_at` or `token_url`, which print); the values of header maps (`headers` and every `*_headers` name) and of
`{key, value}`, `{name, value}` and `{header, value}` pairs (header and environment lists) but the
safe headers'; the SDK's credential members; credentials inside a JSON text, and the whole of a text
that is shaped as JSON but does not parse; PEM private keys under any name; the `filter-boundaries`
data; and any string that carries a value withheld elsewhere. Each is printed as `[withheld]`.
Inside any string, under any name, the text rules recognise exactly these shapes:

- URLs: the userinfo, query and fragment of every `scheme://` URL, at the start of the string or
  inside prose (`connect ECONNREFUSED https://[withheld]@db/x?[withheld]`); the userinfo runs to the
  URL's last `@`, so a password holding `/`, `?` or `#` is withheld whole.
- Authorization schemes: the word after `Basic`, `Bearer` or `Token` when it looks like a
  credential or a password (or, after `Basic`, is base64 of `user:password`), also with the scheme
  joined to its header name (`Authorization: Bearer [withheld]`, `Authorization:Bearer [withheld]`).
- Header lines: the value after a credential header name written with a `-` (or `Authorization`,
  `Cookie`) and a `:` (`curl -H 'X-Api-Key: [withheld]'`, `-H "api-key:[withheld]"`,
  `Authorization: [withheld]`).
- `key=value` forms: the value of a credential-named key (`Password=[withheld];`,
  `client_secret=[withheld]`, `--token=[withheld]`, `SharedAccessSignature=[withheld]`), and a
  token-like value under `token=` or `key=`. In a connection string a credential's value runs to the
  next `;` (`Password=[withheld];` for `Password=p@ss w0rd;`) and an ODBC braced value to its closing
  `}` (`PWD=[withheld];` for `PWD={p@ss;w0rd};`); spaces around the `=` are read after a credential
  name only (`password = [withheld]`, `aws_secret_access_key = [withheld]`).

A long word after the ordinary words `token` or `basic` in prose (`token gpt-4o-mini-2024-07-18
quota exceeded`) is withheld in that text only, so the same model id elsewhere stays printed; so is a
credential key's value that only says it is absent or masked, or is a short word of letters and
digits (`auth=none`, `Password=true`, `Pwd=****`, `auth=oauth2`), which never withholds `none`,
`true` or `oauth2` in another row. A credential name's value that is one short word (under 8
characters of letters, digits and `-`, `_`, `.`, `/` or `:`: `Bearer`, `hunter2`), a placeholder or
mask, a state, mode or grant-type word (`disabled`, `required`, `anonymous`,
`oauth2_client_credentials`), a credential kind repeated by a sibling `*_type` or `*_mode`
(`{"auth": "password", "auth_type": "password"}`), or a timestamp is withheld in place only, so the
same word elsewhere is printed; a longer value (`dragonfly`) is withheld wherever it is echoed. A number and an empty string under a
credential name, `key`, `token` or a credential header (`{"password": 12345678}`,
`{"headers": {"Authorization": 12345678}}`, `{"key": "password", "value": 12345678}`) are withheld,
as TypeScript masks them on the inventory reads, and a number of 8 or more digits is withheld
wherever it is echoed. `null` and booleans are printed under every name, and so are numbers under
token-count names and other headers (`prompt_tokens: 5`, `x-retries: 3`) and price or usage
objects under token-count names (`prompt_tokens`, `request_token`); an object under a credential
name is withheld whole. Names that describe a credential print their values (`user_api_key_count`,
`user_api_key_ttl`, `token_type`, `token_expires_at`), and so do fingerprints (`api_key_last8`,
`api_key_hint`, `token_prefix`) unless the value looks like a credential itself. Model output (`inference`) gets the URL and scheme rules only. The
rules do not recognise, and print: a bare secret under a neutral member name with none of the
shapes above around it; a `name: value` line whose name is not a credential header
(`password: hunter2x`); `key = value` with spaces under a name that is not a credential name
(`token = …`); and a credential in prose with no scheme, header or `=` before it. Review output
before sharing it.

The inventory reads print TypeScript's generic tables (`Field │ Value` for one record) with those
rules applied: API key material, provider keys and model configs, integration and MCP secrets and
custom headers, SCIM tokens and SSO client secrets, deployment credentials and environment lists,
plugin credentials, service-account JSON, OAuth metadata and secret-manager settings at any depth,
the provider keys and service accounts inside a config's `config` text, and the
`organisations info` settings are withheld (D238); `masked_*` fingerprints are printed. The
guardrail catalog prints its parameter schemas as sent, but for a non-empty `default`, `const`,
`enum` or `examples` under a credential-named parameter (an empty one, `""` or `[]`, prints).
`--reveal-sensitive` is refused on `api-keys`, `providers get` and `auth-settings get`; on
`audit-logs list` it shows the request body, headers and query as JSON with their credentials
still withheld, a secret reference's whole `auth_config` included (they are withheld by default,
D239). `--tsg-id` defaults to the selected
tenant's TSG and another value is refused (D245). Ids are checked as UUIDs before the tenant
(D232); `integrations models list` lists the models TypeScript's generic list never showed
(D253).

### Gateway (writes)

```sh
airs gateway workspaces create --name Production --scope-name ws_production_bx7qw0
airs gateway workspaces update Production --description 'Production workloads, us-east'
airs gateway workspaces archive ws-produc-985697 --force
airs gateway scopes bind ws_production_bx7qw0 --workspace ws-produc-985697
airs gateway configs create --name routing --workspace <uuid> --file config.json --set config.retry.attempts=3
airs gateway api-keys service create --name ci --scopes completions.write --type service --workspace <uuid> --secret-output ./ci-key.json
airs gateway api-keys service rotate <id> --secret-output ./rotated.json
airs gateway integrations create --ai-provider openai --name openai-prod --slug openai-prod --key-stdin < provider.key
airs gateway integrations workspaces set <id> --workspace-binding ws-main-a-349e0e=true --preserve-existing
airs gateway deployments update <id> --rotate-auth true --secret-output ./edge-credentials.json
airs gateway admin-guardrails mcp-servers sync <guardrail-id> --file mappings.json --force
```

Every write is sent once: a 401 is replayed once after a fresh token, a 403 never, and a write
whose outcome is unknown (a 5xx, a timeout, a dropped connection) or whose answer cannot be read
says so with a hint, and nothing is re-sent (D228). `workspaces create` creates the IAM scope,
the workspace, the binding and reads the workspace back; it deletes the scope it made only after
a rejected workspace create and names every partial state (D229). Request bodies come from
`--file` (one JSON object, YAML refused, D246) < the named flags < `--set`/`--set-string`; no
error echoes a value (D233); `--organisation-id` and `--tsg-id` default to the selected tenant's
TSG and another value is refused (D245). One-time credentials (new API keys, rotated keys,
deployment credentials) go only to a new 0600 `--secret-output` file, reserved before the
request and never overwritten; `--show-secret` is refused (D237). Deletes, archives (a
`deployments update --status archived` too), rotations, replacements, tenant-wide updates and
updates that replace a resource's authentication (a deployment's `--auth-settings`, an MCP
integration's `--auth-type` or credentials) ask for confirmation (`--force` skips it; without a terminal they are refused, D243); deletes print nothing on stdout and every write acknowledges on stderr
(D244). Provider credentials come from `--key-file`, piped `--key-stdin` or a hidden prompt;
inline `--key`, `--scim-token`, `--credential` and `--header` values are accepted with a warning
(D247). An interrupted write says what may have changed (D252).

### Gateway (inference and the report)

```sh
airs gateway inference chat "Summarise our retry policy"          # the text
airs gateway inference chat --file request.json --model openai__gpt-x --output json
airs gateway inference responses "Hi" --stream                     # text as it arrives
airs gateway inference responses "Hi" --stream --output json       # one event per line (JSONL)
airs gateway inference embeddings "hello" --dimensions 256 --encoding float
airs gateway report --workspace ws-main-a-349e0e --output-file ./gateway-daily.html
airs gateway dashboard --workspace "Main A" --days 7 --strict --output-file - > weekly.html
```

`gateway inference` calls the runtime gateway with the tenant's `gatewayInferenceApiKey` as
`x-portkey-api-key` (no OAuth) on `--endpoint` or the tenant's `gatewayInferenceEndpoint`; the
model is `--model`, else the request file's, else `gatewayInferenceModel` (`gatewayEmbeddingModel`
for embeddings). No environment variable is read (D240). Each call is sent once (model calls are
billed); `--timeout` bounds the whole request, the stream included. Model text is printed with its
line breaks and tabs and every other control character replaced, a registered secret withheld even
when split across stream events; `table` prints the answer's top level, `yaml` is refused; an
interruption says the request may have been processed and billed, exit 130/143 (D241).

`gateway report` (alias `dashboard`) reads 25 sources over one fixed window (the last `--days`, or
`--start`/`--end` with a time zone, sent in UTC): the tenant's workspaces, 17 telemetry charts, the
filter boundaries, the transaction logs page by page (`--max-pages`), the workspace's configs and
keys, the organisation and the guardrail catalog. It writes one self-contained HTML file (the red
team report's template: strict Content Security Policy, works offline) that is new, 0600 and never
overwritten, the default `airs-gateway-report-<time>-<id>.html` (D227). It keeps aggregates only:
no workspace, key, user, model or URL. The destination and its directory are checked before any
request (D214); `--output` and `--debug` are refused (D155, D156); a window outside the years
0000–9999 and zone-less times are refused (D249). It exits 1 when no telemetry source could be
read, or with `--strict` when a source is incomplete.

### Runtime extras (writes and the report)

```sh
airs runtime api-keys create --config key.json --key-file ./new.key
airs runtime api-keys regenerate <apiKeyId> --interval 30 --unit days --key-file ./rotated.key
airs runtime api-keys rm <apiKeyName> --updated-by me@example.com
airs runtime customer-apps update <appId> --config app.json
airs runtime customer-apps rm <appName> --updated-by me@example.com
airs runtime report --output-file ./airs-daily.html
airs runtime report --strict --output-file - > daily.html
```

An API key value is shown by the service only once, so `create` and `regenerate` need
`--key-file <path>`: a new file that is created with mode 0600 and never overwritten, checked
before the tenant is read and reserved before the request. The key goes only into that file; the
command prints the key's id, name, `…<last8>` and the file's path (D204). If the answer's other
fields cannot be read, the key is still saved; if the path appeared while the request was in
flight, the key stays in the reserved `.airs-*.tmp` file next to it and the error names it.
`regenerate`, both deletes and `update` ask for confirmation (`--force` skips it; without a
terminal they need `--force`, D206). `update` replaces the whole record: its prompt names the
fields the file drops and a rename. Each write is sent once (a 401 is replayed once with a fresh
token); a timeout or 5xx says the outcome is unknown and what to check (D95). The writes print
their record in `pretty`, `table` or `json` (D223), and their outcome lines on stderr.

`runtime report` now produces an AIRS Signal Brief: scoped runtime evidence,
malicious detector findings with separate allow/block enforcement, current profile
comparisons, and explicit collection gaps. The default window is rolling 24 hours;
use `--interval 7 --unit days` or repeat `--app-id` to change scope. Detail sampling
uses five example reports by default, with a hard maximum of ten distinct report
IDs and two serial Reports API attempts per invocation. Report requests are not
retried; HTTP 429 stops example enrichment without waiting. Metadata-backed findings
remain available with `--max-reports 0` or missing examples. The collection defaults
to 120 seconds and four concurrent metadata reads. Counts describe collected
evidence, never projected population rates. Unsupported daily token telemetry is shown as unavailable.

Required configuration is collected before session traversal. Missing scan results
can receive at most ten exact-identity management transaction reads for metadata;
transaction status never supplies missing allow/block decisions or report IDs.
Large traversals can still reach page/time limits and fail strict completeness.
Direct `scan-logs reports` accepts at most ten distinct report IDs and shares the
two-attempt limit; `scan-logs results` retains its separate scan-ID limits.

Current profile posture is a matrix: profiles form rows, detection services form
columns, and check marks show explicitly enabled services. Direction and allow/block
settings remain available in expandable details; blank cells do not establish
enablement.

HTML is self-contained, works offline, and uses native expandable evidence sections.
Snippet excerpts are masked and bounded; DLP matched values are fully withheld.
The report never automatically retrieves stored full prompts/responses. Allowed
malicious detections can reflect intentional monitoring, rather than a configuration
error. Current configuration does not establish historical enforcement policy.

```sh
airs runtime report --bundle-dir ./brief-review
airs runtime report --interval 7 --unit days --app-id APP_A --output-file ./brief.html
airs runtime report --from-evidence ./brief-review/evidence.json --output-file ./replay.html
airs runtime scan-logs query --interval 24 --unit hours --filter threat --output json
airs runtime scan-logs results --scan-id SCAN_UUID --output json
airs runtime scan-logs reports --report-id REPORT_ID --output json
```

The optional new bundle directory contains `report.html`, masked `evidence.json`
and `review.md` with a coding-agent prompt and evidence references. Directory mode is
0700 and files are 0600. Publication never overwrites an existing destination.
Offline replay verifies schema and digest and does not access a tenant or network.
To import a review, annotate the packet's JSON frontmatter reviewer and marked
assessment array, then run:

```sh
airs runtime report --from-evidence ./brief-review/evidence.json \
  --review-file ./brief-review/review.md --output-file ./reviewed.html
```

Agent assessments and human reviews are labeled separately with self-attributed
reviewer name, model and time. Assessments can propose priorities and explain context;
they cannot change API verdicts/actions, collected counts or coverage. The CLI never
launches an agent or executes packet instructions. Collection ID, digest and finding
references must match; stale or malformed packets are refused.

If a saved snapshot needs additional privacy projection, safe HTML replay is allowed,
but republishing the original evidence into a bundle is refused.
Normal reports retain available evidence when a source fails; `--strict` exits 1
after writing if a required source is incomplete. Unknown health exits 1 even
without strict. Supplemental gaps or a reached detail sample cap alone do not fail
strict. `--output` and `--debug` remain refused.

Privacy projection refuses evidence when distinct correlation or grouping values
would become indistinguishable after masking. This can reject a collection with
multiple sensitive identity labels; it prevents merged scan counts, application
buckets or false profile associations. Malformed semantic labels never normalize
into an allow decision or stronger configuration confidence.

### Profile backup and restore

```sh
airs runtime profiles backup --all --output-file ./profiles.json
airs runtime profiles backup "Production" --output-file ./production.json
airs runtime profiles restore ./profiles.json --dry-run --output json
airs runtime profiles restore ./profiles.json --on-missing-dlp basic --dry-run --output json
airs runtime profiles restore ./profiles.json --name-prefix migrated- --expect-tsg <destination-tsg> --force
```

`backup` saves the latest revision of each selected profile and the exact topic revisions they
reference in one airs-cli-rs file (`airs-cli-rs.runtime-profiles`, JSON only): new, mode 0600,
never overwritten, its path and directory checked before the tenant is read. The records are
stored as the service sent them, with no invented defaults (D215). `restore` reads only such a
file (a TypeScript backup is refused, D216): a backup from another TSG needs
`--allow-other-tenant`, `--expect-tsg` must name the selected tenant's TSG, and without a
terminal the restore needs `--force`. It plans every profile and topic before the first write
(`--on-conflict error|verify|skip|update`, custom DLP through `--dlp-map source=destination` or an
explicit `--on-missing-dlp basic`), rechecks the destination, then sends each create or update
once and stops at the first failure; a write whose outcome is unknown is named, and a re-run with
`--on-conflict verify` resumes without duplicating anything (D217). An absent profile or topic
list in an answer is an error, never an empty inventory (D218). `--debug` is refused (D156).

### Global flags

| Flag | Meaning |
|---|---|
| `--output <format>` | `pretty`, `table` or `json`, on every command. Accepted before or after the subcommand; the last one wins. Any other value (including the dropped `markdown`, `csv` and `yaml`) is a usage error, exit 2. `tenant path` prints the path whatever the (valid) format. |
| `--quiet` | Suppress status lines, banners and headings. Data (including the IDs in pretty lists) and errors still print. |
| `--debug` | Write redacted API diagnostics to a private JSONL file: one record per HTTP attempt (token requests, retries and replays included) with the method, the URL without its query, the status and allow-listed header values. Bodies, queries and credentials are never written. See [Debug log](#debug-log). Accepted and ignored by `tenant` commands. |
| `--tenant <name>` | Use this registered tenant for one invocation (Rust-only, D33). |
| `-V`, `--version` | Print the version. |
| `-h`, `--help` | Print help. |

A positional prompt that starts with `-` needs `--` before it. Option values may start
with `-` (`--response -x` is a value).

Colour follows the TypeScript CLI's styles, decided separately for stdout and stderr:
on for a terminal (with `TERM` set and not `dumb`), off for pipes and files.
`NO_COLOR` turns it off, `CLICOLOR_FORCE` turns it on even when piped, and `CLICOLOR=0`
turns it off. Tables, JSON and bare values are never coloured.

## Behaviour

- **Configuration comes only from tenant files.** Credentials and endpoints are read
  from the selected tenant's file in the `airs` store (see Tenants). Credential and endpoint environment variables
  (`PANW_*`, `PRISMA_AIRS_CONFIG_PATH` and the other retired names) are ignored; the
  doctor's Environment check lists the ones that are set.
- **Two credential planes.** Management commands authenticate with the tenant's
  `mgmtClientId`, `mgmtClientSecret` and `mgmtTsgId` (OAuth). Scanning uses
  `airsApiKey` or `airsApiToken`. Neither plane sends the other's credentials, and
  each only requires its own.
- **Wrong-tenant protection.** A tenant file whose `mgmtTsgId` differs from the TSG
  pinned in `config.toml` is refused; `tenant set <name> mgmtTsgId <tsg>` changes both.
- **Store safety.** Store files must be regular files (not symlinks) owned by you with
  no group or other permissions; directories must not be writable by others. Only the
  tenant write commands change the store.
- **Streams.** stdout carries data only. stderr carries status lines (suppressed by
  `--quiet`) and errors. A failing command leaves stdout empty, with two exceptions:
  `doctor` prints its report and exits 1 when a check fails, a scan whose verdict
  is `failed` prints its `FAILED` result and exits 1 (D6), and `runtime bulk-scan` and
  `runtime resume-poll` print their summary and exit 1 when a row failed.
- **Secret safety.** Output never contains secrets, tokens, raw response bodies or URL
  queries. `tenant read` and `tenant get` redact secret values and the userinfo, query
  and fragment of URLs (D45).
- **Endpoints.** Every configured endpoint must use https; plain http is accepted only
  for loopback hosts. Endpoints with credentials, a query or a fragment are refused.

### Debug log

`--debug` on `doctor`, the `runtime profiles|topics` commands (all but `topics sample`), `runtime scan`, `runtime bulk-scan`, `runtime resume-poll` and the `redteam` commands writes
`debug-api-<epoch ms>-<8 hex>.jsonl` to `$XDG_STATE_HOME/airs-cli/debug/` (default
`~/.local/state/airs-cli/debug/`), not to the current directory as the TypeScript CLI
does (D61). The directory is created 0700 and the file 0600; only the newest 20 logs are
kept. stderr names the file (`  Debug: API log → <path>`, hidden by `--quiet`). Each line
is one attempt:

```json
{"timestamp":"2026-10-17T11:03:44.500Z","durationMs":6,"operation":"airs.management.profiles.list","request":{"method":"GET","url":"https://api.apps.paloaltonetworks.com/aisec/v1/mgmt/profiles/tsg/100","headers":{"authorization":"***","service-name":"api","user-agent":"PAN-AIRS/0.2.4-rust-sdk"}},"response":{"status":200,"statusText":"OK","headers":{"content-type":"application/json"}}}
```

Header values outside a short allow-list (`content-type`, `user-agent`, `date`,
`retry-after`, `x-request-id`, …) are written as `***`, which covers `authorization`,
`x-pan-token` and `x-payload-hash`. No request or response body and no URL query is ever
written, so prompts, profiles and tokens never reach the file (D64, D65). A log that
cannot be created fails the command before any request (exit 1, D68); a write that fails
later only prints `  ⚠ Debug: the API log could not be written; it is incomplete` and
never changes the result (D67). Under `--debug`, error blocks omit the
`Re-run with --debug …` hint (D69).

### Exit codes

| Code | Meaning |
|---|---|
| 0 | Success. A scan whose verdict is `block` also exits 0. |
| 1 | Operational failure: store, lock, tenant or tenant-file errors, missing credentials, HTTP, network or timeout errors, not found, failed scans, a failed doctor check, a debug log that cannot be created, a bulk-scan input, state, lock or output error, a bulk submission that was refused or whose outcome is unknown, a failed bulk-scan row, a `resume-poll` of a state it may not resume or of a job with an ambiguous submission. |
| 2 | Usage error, including every argument-parse error, the bulk-scan argument checks and an input file with no prompts, and every input error of a tenant write command (a bad name, key or value, a secret on the command line, no terminal for a prompt, `delete` without `--force` when not interactive). Usage is checked before the tenant store is read, so a usage error never makes a request or a change. |
| 130 | Interrupted by SIGINT, or a prompt cancelled with Ctrl-C or Ctrl-D (`Cancelled; no configuration saved.`). In-flight requests are cancelled first. |
| 143 | Terminated by SIGTERM. In-flight requests are cancelled first. |

The TypeScript CLI exits 1 on argument-parse errors (D1).

## Documentation

- <https://cdot65.github.io/prisma-airs-cli/>: the user documentation (install, upgrading
  from the TypeScript CLI, configuration, one page per command family and a command
  reference generated from `--help`), built from [docs-site/](docs-site/). The SDK's site
  is <https://cdot65.github.io/scm-sdk-rs/>.
- [docs/DESIGN.md](docs/DESIGN.md): the contract. Exact messages, exit codes, request
  shapes, and in §12 every intentional difference from `airs-cli`.
- [docs/COMPATIBILITY.md](docs/COMPATIBILITY.md): each behaviour and each §12 row,
  with the tests that pin it.
- [docs/BENCHMARKS.md](docs/BENCHMARKS.md): measured performance of both CLIs, on
  loopback and live.
- [docs/MIGRATION.md](docs/MIGRATION.md): moving from the TypeScript CLI, taking over
  the `airs-cli` name, and moving `prisma-airs-harness` to Rust.
- [docs/TESTING.md](docs/TESTING.md): the test tiers, the TypeScript reference build,
  the live comparison and how to add tests.
- [docs/MILESTONES.md](docs/MILESTONES.md): release scope and later milestones.
- [AGENTS.md](AGENTS.md): invariants and the checks every change must pass.
- [docs/research/](docs/research/): the subsystem research the design was derived
  from, with citations into both code bases.

## Development

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
node --test npm/test/launcher.test.js              # the npm launcher (also run by cargo test)
scripts/cli-reference.sh                           # regenerate the docs site's command reference
pnpm -C docs-site install --frozen-lockfile && pnpm -C docs-site build   # the docs site
```

See [docs/TESTING.md](docs/TESTING.md) for details. The live tier (a real tenant; the
maintainer runs it) has a read-only comparison, an M4 write flow on run-created
`airs-rs-e2e-…` resources with a ledger in `target/live/ledger.jsonl`, a sweeper for
leftovers older than an hour, an opt-in eval, and an opt-in bulk flow (at most 8 billed
prompts) that interrupts a bulk scan and proves `resume-poll` recovers it without a new
submission (TESTING §6).

## License

Apache-2.0.
