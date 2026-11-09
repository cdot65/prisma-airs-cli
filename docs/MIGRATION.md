# Migration: replacing the TypeScript `airs-cli`

> **`tenant import` was removed after 8.2.** Where this page describes it, use version 8.2 of
> `airs` to run the import, or register tenants with `airs tenant create`.

`airs-cli-rs` replaces the TypeScript CLI outright. No TypeScript is installed
alongside it, and there is no dispatcher. From 8.0 it lives in the repository that held
the TypeScript CLI, `cdot/prisma-airs-cli`, ships on npm as `@cdot65/prisma-airs-cli`
(a launcher for prebuilt binaries) and answers to `airs-cli` as well as `airs` (DESIGN
§21.15–§21.17, D330). The scope, drops, renames and roadmap are
in [MILESTONES.md](MILESTONES.md); performance data is in [BENCHMARKS.md](BENCHMARKS.md).
This page covers what a user and the `prisma-airs-harness` have to change.

Paths are abbreviated:

| Prefix | Path |
|---|---|
| TS | this repository's history at tag `v7.3.0` (`7a54e46`, 7.3.0, the final frozen reference from 8.0; before it `5711012`, 7.2.1-forgejo.0, and until 8.0 the separate checkout `/Users/cdot/development/redtail/prisma-airs-cli`) |
| H | `/Users/cdot/development/cdot65/prisma-airs-harness` |
| S | `/Users/cdot/development/redtail/scm-sdk-rs` |

## 1. Where things stand

The TypeScript CLI has 279 leaf commands. After the drops, the remaining parity
target is every API-backed command, plus `--debug`, reports, backups and restores,
and `model-security install`. Release 0.1 ports 10 of them:

- `tenant list/path/read/get`
- `doctor`
- `runtime profiles list/get`
- `runtime topics list/get`
- `runtime scan`

`scm-sdk` 0.1.0 already provides the product APIs for nearly all the rest, through
its `management` (which includes `dlp`), `red-team`, `model-security`, `gateway`,
`iam` and `agentguard` features. What is left to write is CLI logic. The roadmap
ports it in 10 minor releases and renames the executable to `airs` at 1.0 (DESIGN §21).

## 2. Moving from the TypeScript CLI

### 2.1 Steps for a user

1. Install the Rust CLI (`airs`) through one channel: Homebrew (`brew install airs-cli` from
   the `cdot65/prisma-airs-cli` tap), npm from 8.0.0 (`npm install -g @cdot65/prisma-airs-cli`,
   which replaces an npm 7.x install in place), crates.io (`cargo install --locked
   airs-cli`) or a Forgejo release tarball (signed and notarised for macOS, static musl for
   Linux). Mixing channels shadows one install with another (`airs doctor` warns, D331).
2. After upgrading to 0.3, run `airs tenant import` once (DESIGN §3.12).
   - `airs tenant import --dry-run` first shows what would happen: one row per
     tenant with `import`, `replace` or `skip` and notes naming renamed, dropped and
     unknown keys (never values). Nothing is written.
   - `airs tenant import` then copies every tenant into `~/.config/airs-cli/`
     (`$XDG_CONFIG_HOME/airs-cli/`, or `$AIRS_CLI_CONFIG_DIR`): `config.toml` plus
     `tenants/<name>.toml`, files 0600 and directories 0700. The TypeScript registry
     and configs are read, never modified. The TypeScript selection is kept when
     airs-cli-rs has none yet.
   - It reads the registry the TypeScript CLI uses (`PRISMA_AIRS_TENANTS_PATH`, else
     `$XDG_STATE_HOME/prisma-airs/tenants.json`, else
     `~/.local/state/prisma-airs/tenants.json`); `--from <registry>` names another one,
     and `--from-config <file> --name <name>` imports a single JSON config.
   - Existing airs-cli-rs tenants are never overwritten without `--replace`, and a
     name that differs only in case from an existing one is skipped (D85). A tenant
     whose config cannot be read, or whose `mgmtTsgId` differs from its registration,
     is skipped and the command exits 1 after importing the others (`--dry-run` exits 1
     in the same case, so it doubles as a check).
   - `--output json` gives scripts the rows `{name, tsgId, action, notes}`.
   - From 0.3 no other command reads the TypeScript files, so changes made later with
     the TypeScript CLI are not seen; re-import with `--replace` if needed. Until you
     import, `doctor` fails its Tenant check with a hint to run the import, and the
     "no tenant selected" messages say the same. Before 0.3, `airs-cli-rs` read the
     TypeScript registry directly, read-only.
3. Finish or abandon any TypeScript bulk-scan jobs. Their checkpoints aren't
   imported.
4. Re-take any TypeScript backups you need with the Rust CLI. TypeScript backup files
   aren't read.
5. Update scripts for the changes in §2.2.
6. Uninstall the TypeScript package from every Node version
   (`npm uninstall -g @cdot65/prisma-airs-cli`, and `pnpm` if it was used there), unless
   npm is the channel you keep: then `npm install -g @cdot65/prisma-airs-cli@latest`
   (8.0.0 or later) replaces it with the Rust launcher. The 8.0 package exports no
   JavaScript library.
7. From 1.0 the Rust executable is `airs`, with `airs-cli-rs` as a compatibility name
   (DESIGN §21.2). In 1.x there was no `airs-cli` name, so the TypeScript CLI's `airs-cli`
   was never shadowed. From 8.0 `airs-cli` is a name of the Rust CLI too (D330): it prints
   exactly what `airs` prints, so 7.x scripts that run `airs-cli` get the Rust behaviour
   described in §2.2.

### 2.2 What changes for scripts

Every row is pinned by a test listed in [COMPATIBILITY.md](COMPATIBILITY.md) §4.

| Change | Detail |
|---|---|
| Renamed command groups | `aigateway` → `gateway` and `agentguard` → `supplychain`. Config keys are renamed too, and no aliases exist. |
| Dropped commands | `completion`, `redteam judge`, `runtime dlp generate` |
| Dropped formats | `--output markdown\|csv\|yaml` is a usage error (exit 2). A tenant `defaultOutput` that names one of them makes the config invalid. |
| Directional profiles (8.0, TS 7.3.0) | `runtime profiles create\|update` and `topics apply\|revert` take `--direction` and `--ai-profile-index` (decimal digits only, D332); the `--no-…` flags and `--[no-]enable-full-conversation-inspection` exist; `profiles create --config` takes the name from the file and refuses `--name`; `--config` is exclusive with every write flag. A create with no policy flag is refused (TS 7.3.0 creates an empty policy, D102); `revert` keeps a still-referenced topic with exit 0 (TS fails, D99). A flag create with `--direction` also writes the other three directions as empty objects, since the service refuses a directional profile without all four (TS 7.3.0's body is refused, D335), and one whose flags fill no direction (shared settings only) is refused before OAuth, exit 2 (TS sends it and the service refuses it, D336). Accepted live for all four directions (DESIGN §13.17); supported, for single-entry profiles. Read the known limitations (§2.3), `dlp_tenant_id` on writes included. |
| Version | `--version` reports `0.N.0` until 1.0, `1.0.0` for 1.0, and from 8.0.0 the one version shared by every channel |
| Exit codes | Argument-parse errors exit 2, not 1, and usage is checked before the tenant is read (D1, D2, D18). Signals give 130/143. |
| Scans fail closed | A failed scan prints `FAILED` and exits 1; TypeScript printed `ALLOW` and exited 0 (D6). Response detections count (D7). A scan is never re-sent (D9). |
| Environment | No `PANW_*` credential fallback (D8). Timeout and debug variables are ignored (D31). `HTTPS_PROXY` and macOS system proxies are honoured (D36). Plain-http endpoints other than loopback are refused (D48). |
| Doctor | 7 checks (8 from 0.10, 9 from 8.0: `Executable` first, D331); server errors fail and rate limits warn (D19–D25). Each probe makes one attempt (D49). |
| Error text | Built from SDK metadata. Secrets and tokens are withheld (D11, D37, D38, D47). |
| Pagination | A walk that can't finish fails closed instead of truncating (D15, D16, D40, D46). |
| State locations | Tenants live in `~/.config/airs-cli/` (0.3, D70); `AIRS_CLI_CONFIG_DIR` moves them, and `PRISMA_AIRS_TENANTS_PATH` no longer selects tenants. Bulk-scan state and debug logs live in `~/.local/state/airs-cli/`. |
| Tenant keys (0.3) | `aiGw*` keys are `gateway*`, `agentGuard*` keys are `supplychain*`; `dataDir` and the `typesafe*` keys are gone; unknown keys make a tenant file invalid; numbers are plain integers (D71, D72). `tenant import` converts all of this. |
| Tenant commands (0.3) | `tenant create --config` is gone: use `tenant import --from-config <file> --name <name>` (D84). `tenant delete` also deletes the tenant's file (D80). `tenant set <name> mgmtTsgId <tsg>` changes the TSG (D83). Success lines go to stderr and `--quiet` hides them (D74). Input errors exit 2 (D75); values are checked strictly (decimal integers, https endpoints, no empty values, D76). Hints name `airs-cli-rs` (D73). Names that differ only in case are refused (D85), and lists are in byte order (D86). |
| Concurrent changes (0.3) | Writers wait up to 5 s for each other and name the holder; a crashed writer never leaves a lock behind (D77). |
| `--debug` (0.2) | The log goes to `~/.local/state/airs-cli/debug/` (or `$XDG_STATE_HOME/airs-cli/debug/`), not the current directory, and only the newest 20 are kept (D61). It records every request, status and allow-listed headers only: no bodies and no URL queries (D62–D66). `tenant` commands and usage errors create no log (D63). |

### 2.3 Known limitations of directional profiles (8.0)

Directional profiles are **supported, for single-entry profiles**: a live run (2026-10-22)
accepted every direction for creates, edits, topics, backup and restore.

- **Not verified live:** profiles with several AI entries (`--ai-profile-index`), DLP blocks
  inside a direction (`--dlp-action`, `--dlp-profiles`), and the runtime report's
  `Protection configurations` column. The offline tests cover them.
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

## 3. `prisma-airs-harness` to pure Rust

### 3.1 What the harness uses today

No Rust code in the harness calls a Prisma AIRS product API or runs `airs-cli`:

- The native binary refuses `airs cli` and points users to the npm launcher (H `codex-rs/cli/src/main.rs:179-190,1216-1225`).
- Its own HTTP traffic goes to the AI Gateway, OIDC and TypeSafe `GET /v1/models`.

Node handles every AIRS integration:

| ID | Integration | Where | Uses |
|---|---|---|---|
| I1 | `airs cli …` runs a bundled TypeScript CLI 7.1.5 with SDK 0.33.0 | `npm/airs-harness/lib/launcher.js:37-39`, `lib/prisma-cli.js:13-72` | the whole TypeScript CLI |
| I2 | Agent shell access via `AIRS_MANAGED_CLI` and the `managed-cli/airs-cli` shim | `lib/launcher.js:11-26,62` | the whole TypeScript CLI |
| I3 | Nine embedded skills tell the agent to run `"$AIRS_MANAGED_CLI"` | `codex-rs/skills/src/assets/samples/prisma-airs-*/SKILL.md` | about 100 commands, the doctor JSON shape, exit codes |
| I4 | Shell completion merges the product command tree | `lib/completion.js:55-72` | `completion` |
| I5 | Red-team ASR judge | `prisma-airs-asr-judge/scripts/asr_judge.mjs`, `cli/src/airs_typesafe.rs:27-29,277-284` | `redteam judge --harness-credentials` with TypeSafe; exit 4 |
| I6 | MCP scanner service `pan_inline_scan`, TypeScript SDK 0.21.0, Node 22 distroless image | `mcp-scanner/src/server.ts:56-97`, `failure.ts:28-49` | one `syncScan` per tool call (**hot path**) |
| I7 | CI validation | `scripts/validate_prisma_cli.py:67-185` | doctor rows including `AI Gateway API`, `tenant create/switch`, 20 help contracts, `dlp generate` |

### 3.2 Rust replacements

| Need | Replacement | Status |
|---|---|---|
| I6 MCP scanner | `scm-sdk` in its own service. Client from `scm::airs::scan::Scanner::for_region(Region::Us, Some(key), None)`. Call `sync_scan` with `SyncScanOptions { metadata, request: RequestOptions::with_timeout(10s)?.with_retries(0)? }`. MCP server on rmcp, which is already in the harness workspace. | **Ready now.** The SDK never re-sends a scan, which matches the current `numRetries: 0`. Keep the allow/block-only check and the `scan_id` check. |
| Release-1 commands in I1–I3 | `airs-cli-rs` | Ready. The skills must state D50/D51 (only `pretty`, `table` and `json`), D6, D1 and D9. |
| `tenant create/switch/set/unset/delete` | 0.3 (M4t), on the new TOML layout; `tenant import` copies the TypeScript tenants (the harness's validator should call `tenant create`/`switch` on `airs-cli-rs` and drop `--config`) | Ready in 0.3 |
| `doctor` row `AI Gateway API` | Becomes "Gateway API" in 0.10. `completion` and `AIRS_CLI_INVOKED_AS` are not planned. | Gap / dropped |
| Topic writes, bulk scan, Red Team, Model Security, gateway, supplychain | 0.4–0.11 (MILESTONES.md). The `scm-sdk` APIs exist. | Gap |
| DLP CRUD, profile writes, runtime extras, `topics apply/eval/revert/sample` | 0.4, 0.7 and 0.9 | Gap |
| I5 `redteam judge`, `dlp generate` | Dropped from the CLI | Dropped: the harness must replace or retire the asr-judge and dlp-testing skills |

### 3.3 Link `scm-sdk` in-process, or run `airs-cli-rs` as a separate executable?

- **Agent and `airs cli` paths: a separate executable.**
  - This keeps the shell contract and per-command sandbox approval.
  - It avoids a second Tokio runtime and the signal handlers inside the harness.
  - It avoids Cargo feature unification. The harness turns on serde_json `preserve_order` (H `codex-rs/tui/Cargo.toml:98`) and `arbitrary_precision` (`codex-rs/exec-server-protocol/Cargo.toml:23`). `airs-cli-rs` forbids `preserve_order`, because it changes the SDK's request bytes.
  - `airs-cli-rs` keeps its library modules private. It can only be embedded as a multicall entry point before the harness runtime starts. Treat that as a later option.
- **MCP scanner: `scm-sdk` in-process,** in its own service and workspace.
- **A future harness readiness check: `scm-sdk` in-process,** through a custom `scm::transport::Transport` that wraps the harness network policy. Settle the feature-unification question first.

### 3.4 Dependency concerns

| Item | Harness | `scm-sdk` / `airs-cli-rs` | Impact |
|---|---|---|---|
| Rust and edition | 1.95, 2024 | MSRV 1.89, 2024 | OK |
| reqwest | 0.12.28 for the workspace; 0.13.4 via rmcp with no TLS features | 0.13.5 with rustls, http2, gzip and system proxy | Adds the 0.13 TLS stack and `rustls-platform-verifier`. Trust and proxy behaviour differ from the harness's `rustls-native-certs` clients. |
| aws-lc-sys | 0.45.0. Bazel builds it against BCR `aws-lc 5.1.0.bcr.1` (`MODULE.bazel:153,365-378`) | 0.46.0 | Moving to 0.46 may need a newer BCR module. Not verified. |
| Platforms | darwin-arm64, x86_64-musl and aarch64-musl; `hickory-dns` forced on musl (`airs-identity/Cargo.toml:37-40`) | `scm-sdk` qualifies x86_64-musl at runtime, compiles for the others, and does not list aarch64-musl | Qualify aarch64-musl, and the musl DNS behaviour, before shipping there. |
| Bazel | `crate.from_cargo` over `codex-rs/Cargo.lock` | — | Every new crate means regenerating the 1.6 MB `MODULE.bazel.lock`. |

### 3.5 Phases

| Phase | Work | Tests | Exit / rollback |
|---|---|---|---|
| 0 Groundwork | Fix stale pins: skills say 7.1.2 while the package pins 7.1.5; `validate_live_prisma_cli.py` pins 5.2.0. Build `airs-cli-rs` for darwin-arm64, x86_64-musl and aarch64-musl, and run its suite on each (0.2). | `airs-cli-rs` suite on each target | Suite green on 3 targets. Nothing ships. |
| 1 MCP scanner in Rust | New service on `scm-sdk` (`scan`), rmcp and axum/hyper. Port the full contract: constant-time `x-airs-terminal-mcp-key` check, 1 MiB body and 200,000-character field limits, strict schema, profile from the environment, 10 s timeout, zero retries, cancellation, the same fail-closed error text, a bounded failure log line. Run it alongside the Node service. | Port `server.test.ts` against an `Endpoint::loopback` fixture (reuse `airs-test-harness`), plus a test that a scan returning allow without a `scan_id` is an error | The gateway evidence contract (`GATEWAY_MCP_REMEDIATION.md:79-89`) passes. Rollback: point the gateway back at the current image digest. |
| 2 Ship `airs-cli-rs` alongside | Bundle the binary in each native platform package. Export `AIRS_MANAGED_CLI_RS`. Route `airs cli-rs` in `launcher.js` and natively in `main.rs:1216`, so native archives get a product CLI for the first time. Point the release-1 skills at it. Don't route commands between the two CLIs transparently, because the agent would see different behaviour without knowing it. | assert_cmd cases in `cli/tests/`, `node --test` launcher cases, a new `validate_prisma_cli_rs.py` | Both validators pass from installed packages on macOS and Linux x64/arm64. Rollback: drop the new variable. |
| 3 Grow coverage | Follow the `airs-cli-rs` roadmap (MILESTONES.md). After each family ships, switch its skill and extend the validator. | Parity with `AIRS_REQUIRE_PARITY=1` plus a bounded live run per family | — |
| 4 Remove Node from the CLI path | When no skill references `$AIRS_MANAGED_CLI`, remove: the bundled TypeScript CLI and SDK, sharp, pdf-lib and docx, `airs_bundle*`, `managed-cli/`, the dotenv defences, and `asr_judge.mjs` (the judge is dropped) | — | Rollback: the previous pinned harness release. The npm launcher stays while npm is the distribution channel; it then only starts native binaries. |

## 4. Open items for the harness

1. **Dropped commands.** The asr-judge skill and its `redteam judge` entry point, and
   the dlp-testing skill and the CI check that run `runtime dlp generate`, need a
   replacement or removal.
2. **Renames.** The gateway skill, `validate_prisma_cli.py` (its `AI Gateway API`
   doctor row and gateway help contracts) and any `agentguard` references move to
   `gateway` and `supplychain`.
3. **Packaging.** Bundle `airs-cli-rs` as a separate executable (recommended), or link
   it in-process, which first needs the serde_json feature conflict resolved.
4. **Platforms.** Qualify aarch64-musl and the musl DNS behaviour for `scm-sdk`
   (planned in 0.2).
5. **Docs to fix now.**
   - S `README.md:15` still says the crate is "not published to a registry".
   - H `GETTING-STARTED.md:282` documents `airs cli --tenant`, which TypeScript
     rejects.
