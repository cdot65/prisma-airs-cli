# airs-mcp release procedure (M8.4)

The CLI (`airs`) keeps its own release procedure, [RELEASING.md](RELEASING.md): the
Forgejo release, crates.io, Homebrew and npm. This document covers the optional service
(`airs-mcp`), which ships only as a container image. The image is never part of a CLI
package: `airs-cli` packages carry the client in `airs` and never the server, as the
`packages_carry_the_client_in_airs_and_never_the_server` test checks.

Nothing here publishes automatically, and no step is taken merely because tests pass.

## 1. Qualify the source

1. Run the required checks on the release commit:
   ```sh
   cargo fmt --all --check
   cargo clippy --workspace --all-targets -- -D warnings
   AIRS_REQUIRE_PARITY=1 cargo test --workspace
   ```
2. Run the opt-in qualifications in release mode, and keep their output:
   - `qualify_30_and_90_day_reports_on_a_million_scans`;
   - `qualify_restore_and_contention_at_scale`, with `AIRS_QUALIFY_OUT` set.
3. The phase review ledger must show an independent ≥9.0 result (the owner-set threshold, 2026-11-06) for every phase, and M8's
   user-run evidence must be recorded (runbook).

## 2. Build and hash locally

```sh
scripts/m8/build-hashes.sh .local/release-<version>
```

This writes the following:

- `SOURCE-SHA256SUMS`: every tracked file.
- `BIN-SHA256SUMS`: `airs` and `airs-mcp` for each target linkable here.
- `BUILD.txt`: the toolchain versions, the commit, and which targets were built,
  skipped or left to the user.

The script never signs, tags, pushes or uploads.

Platforms this script cannot build:

- **macOS `aarch64-apple-darwin`:** signing and notarisation happen on the release Mac
  through `scripts/release.sh`.
- **The `airs-mcp` container image:** a user step (runbook step 2).

## 3. Image

1. Build the image from `deploy/airs-mcp/Dockerfile` on the release commit. The image
   records the binary's SHA-256 at `/usr/local/share/airs-mcp.sha256`; compare it with
   `BIN-SHA256SUMS` for `x86_64-unknown-linux-gnu`.
2. Push it, then pin the digest in `deploy/airs-mcp/kustomization.yaml`.
3. Commit the pin together with the release notes.

## 4. Deploy

Follow [RUNTIME_MCP_DEPLOYMENT_RUNBOOK.md](RUNTIME_MCP_DEPLOYMENT_RUNBOOK.md). Deployment
is called accepted only with the independent review and the recorded integration
evidence.
