#!/usr/bin/env bash
# M8.4 local build and hash manifest (no publishing). Builds the airs CLI and the
# airs-mcp service in release mode for the host and for each installed Linux target this
# machine can link, then writes, into an empty output directory:
#   SOURCE-SHA256SUMS  sha256 of every tracked source file (git ls-files)
#   BIN-SHA256SUMS     sha256 of every built binary (<target>/<name>)
#   BUILD.txt          rustc/cargo versions, commit, and the targets built or skipped
# It never signs, tags, pushes, uploads or builds container images. Platforms it cannot
# build here (macOS signing/notarisation, the airs-mcp container image) are listed in
# BUILD.txt as user-run steps (docs/RUNTIME_MCP_RELEASE.md).
set -Eeuo pipefail
umask 022
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
OUT="${1:?usage: scripts/m8/build-hashes.sh <empty-output-dir>}"
mkdir -p "$OUT"
[ -z "$(ls -A "$OUT")" ] || { echo "build-hashes: $OUT is not empty" >&2; exit 2; }
OUT="$(cd "$OUT" && pwd)"
cd "$ROOT"
host="$(rustc -vV | sed -n 's/^host: //p')"
TARGET_DIR="$(cargo metadata --no-deps --format-version 1 | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')"
{
  echo "commit: $(git rev-parse HEAD 2>/dev/null || echo unknown)"
  echo "rustc: $(rustc --version)"
  echo "cargo: $(cargo --version)"
  echo "host: $host"
} > "$OUT/BUILD.txt"
git ls-files -z | LC_ALL=C sort -z | xargs -0 sha256sum > "$OUT/SOURCE-SHA256SUMS"
: > "$OUT/BIN-SHA256SUMS"
targets=("$host")
while read -r t; do
  [ "$t" = "$host" ] || targets+=("$t")
done < <(rustup target list --installed 2>/dev/null | grep -- '-linux-' || true)
for t in "${targets[@]}"; do
  if cargo build --locked --release --target "$t" -p airs-cli -p airs-mcp >"$OUT/build-$t.log" 2>&1; then
    for bin in airs airs-mcp; do
      f="$TARGET_DIR/$t/release/$bin"
      [ -f "$f" ] && (cd "$TARGET_DIR" && sha256sum "$t/release/$bin") >> "$OUT/BIN-SHA256SUMS"
    done
    echo "built: $t" >> "$OUT/BUILD.txt"
  else
    echo "skipped: $t (no linker or toolchain here; see build-$t.log)" >> "$OUT/BUILD.txt"
  fi
done
{
  echo "user-run: aarch64-apple-darwin (scripts/release.sh on the release Mac: sign, notarise)"
  echo "user-run: airs-mcp container image (deploy/airs-mcp/Dockerfile; build, push, pin the digest)"
} >> "$OUT/BUILD.txt"
echo "build-hashes: wrote $OUT"
