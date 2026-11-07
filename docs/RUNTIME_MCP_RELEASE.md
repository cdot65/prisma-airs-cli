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
   - `qualify_restore_and_contention_at_scale`, with `AIRS_QUALIFY_OUT` set. This one
     covers the archive only;
   - the coordinated backup at scale: `airs-mcp control-backup-set`, then
     `control-backup-verify`, on a representative archive (the planning size) under the
     Deployment's memory limit (2 GiB). Record the peak resident memory and both
     `backup_set*` lines. Store hashing streams the files, so a multi-GiB archive must
     not need memory proportional to its size.
3. The phase review ledger must show an independent ≥9.0 result (the owner-set threshold, 2026-11-06) for every phase, and M8's
   user-run evidence must be recorded (runbook).

## 2. Build and hash locally

```sh
scripts/m8/build-hashes.sh .local/release-<version>
```

This writes the following:

- `SOURCE-SHA256SUMS`: every tracked file.
- `BIN-SHA256SUMS`: `airs` and `airs-mcp` for each required target that built. It is
  written only when every attempted build succeeded and produced both binaries.
- `BUILD.txt`: the toolchain versions, the commit, and the complete platform matrix.
  Each target is marked built, missing (its toolchain is not installed here) or
  user-run (macOS, the container image). The file always ends with a
  `release-qualification:` line, which reads `blocked (pending: …)` until every
  matrix entry has evidence.

A compiler failure on any attempted target is fatal (exit 1). A required target that is
not installed exits 3. Neither counts as qualified.

The script never signs, tags, pushes or uploads.

Platforms this script cannot build:

- **macOS `aarch64-apple-darwin`:** signing and notarisation happen on the release Mac
  through `scripts/release.sh`.
- **The `airs-mcp` container image:** a user step (runbook step 2).

## 3. Image

1. Build the image from `deploy/airs-mcp/Dockerfile` on the release commit. The image
   records its own binary's SHA-256 at `/usr/local/share/airs-mcp.sha256`.
2. **Record the image binary's provenance.** A host build is not comparable byte for
   byte: the toolchain patch level, the C toolchain and libc, the build path and the
   target all change the output. Record, in the release notes:
   - the release commit;
   - the build stage's base image **by digest**, with
     `docker buildx imagetools inspect rust:1.89-bookworm --format '{{.Manifest.Digest}}'`
     at build time. The Dockerfile names a tag, which moves;
   - `cargo --version` and `rustc --version` inside that image;
   - the target (`x86_64-unknown-linux-gnu`, `--release -p airs-mcp --locked`, the
     workspace `[profile.release]`);
   - the image's `/usr/local/share/airs-mcp.sha256` line;
   - the pushed image digest.
3. **Cross-check, optional and with matching inputs only.** Rebuild in the same base
   image digest at the same path:
   ```sh
   docker run --rm -v "$PWD":/repo -w /repo rust@sha256:<digest> \
     sh -c 'cargo build --locked --release -p airs-mcp && sha256sum target/release/airs-mcp'
   ```
   Equal hashes confirm the image binary. A difference with matching inputs is a
   finding. A host build's `BIN-SHA256SUMS` entry is recorded for the CLI release and
   is **not** compared with the image.
4. Push the image, then pin the digest in `deploy/airs-mcp/kustomization.yaml`.
5. Commit the pin together with the release notes.

## 4. Deploy

Follow [RUNTIME_MCP_DEPLOYMENT_RUNBOOK.md](RUNTIME_MCP_DEPLOYMENT_RUNBOOK.md). Deployment
is called accepted only with the independent review and the recorded integration
evidence.
