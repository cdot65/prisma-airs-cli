# CLAUDE.md

Follow [AGENTS.md](AGENTS.md). It holds the invariants and the required checks.
[docs/DESIGN.md](docs/DESIGN.md) is the behavioural contract. Its §12 lists every
intentional difference from the TypeScript CLI. Issue tracking is on BonBon: see the
`## Issue tracking: BonBon` section of AGENTS.md and `docs/agents/issue-tracker.md`.

This repository, `cdot/prisma-airs-cli`, held the TypeScript CLI through 7.x. From 8.0 it
holds the Rust CLI (crate `airs-cli`, command `airs`); the TypeScript reference the
parity tests compare against is a pinned commit of this repository's own history.

Quick commands (prefix with `export PATH="$HOME/.cargo/bin:$PATH";`):

```sh
cargo build --release                                   # target/release/airs (and the airs-cli, airs-cli-rs copies; native arm64 on Apple Silicon)
cargo test --workspace                                  # unit + loopback executable + parity tests
AIRS_REQUIRE_PARITY=1 cargo test --workspace            # fail if the TS reference build is missing
scripts/build-reference-cli.sh                          # build the pinned TS reference (this repository's history) into .reference/
scripts/cli-reference.sh                                # regenerate the docs site's command reference
AIRS_RS_LIVE=1 AIRS_E2E_TENANT=<name> [AIRS_E2E_PROFILE=<profile>] cargo test --test live -- --nocapture --test-threads=1   # bounded live comparison (DESIGN §10)
```

Layout:
- `src/`: the CLI. See DESIGN §11 for the module map.
- `crates/airs-test-harness`: the loopback fixture server, tenant sandbox and dual-CLI runner.
- `tests/`: executable-level tests.
- `npm/`: the npm launcher package (`@cdot65/prisma-airs-cli`); `scripts/npm-package.sh` stages it from the release tarballs.
- `docs-site/`: the Docusaurus site published at https://cdot65.github.io/prisma-airs-cli/.
- `docs/research/`: the subsystem specs the design was derived from.
