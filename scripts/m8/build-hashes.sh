#!/usr/bin/env bash
# M8.4 local build and hash manifest (no publishing). Builds the airs CLI and the
# airs-mcp service in release mode for every locally buildable target of the release
# platform matrix, then writes, into an empty output directory:
#   SOURCE-SHA256SUMS  sha256 of every tracked source file (git ls-files)
#   BIN-SHA256SUMS     sha256 of every built binary (<target>/release/<name>)
#   BUILD.txt          rustc/cargo versions, the commit, and one line per matrix entry:
#                        built: <target>
#                        missing: <target> (target not installed here)
#                        user-run: <platform> (evidence pending; see the release doc)
#                      then `local-evidence: complete|incomplete` and
#                      `release-qualification: blocked (pending: ...)` while any entry is
#                      missing or user-run.
# It never signs, tags, pushes, uploads or builds container images.
#
# The release matrix (docs/RUNTIME_MCP_RELEASE.md, npm/):
#   x86_64-unknown-linux-gnu   host build, recorded for the CLI release; never compared
#                              with the image's binary (docs/RUNTIME_MCP_RELEASE.md step 2)
#   x86_64-unknown-linux-musl  npm package
#   aarch64-unknown-linux-musl npm package
#   aarch64-apple-darwin       user-run on the release Mac (sign, notarise)
#   airs-mcp container image   user-run (build, push, pin the digest)
#
# The host target is mandatory. A required target whose Rust target is not installed
# (`rustup target list --installed`) is recorded as missing, never as built. Any build
# that fails, or that does not produce both binaries, is a fatal error: a compiler or
# linker failure is never treated as an unavailable toolchain.
#
# Exit codes: 0 every locally buildable required target built and hashed (user-run
# platforms still pending, as BUILD.txt states); 1 a build failed or a binary is
# missing; 2 usage; 3 a required target is not installed here (local evidence
# incomplete; release qualification blocked).
set -Eeuo pipefail
umask 022
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
OUT="${1:?usage: scripts/m8/build-hashes.sh <empty-output-dir>}"
mkdir -p "$OUT"
[ -z "$(ls -A "$OUT")" ] || { echo "build-hashes: $OUT is not empty" >&2; exit 2; }
OUT="$(cd "$OUT" && pwd)"
cd "$ROOT"
REQUIRED=(x86_64-unknown-linux-gnu x86_64-unknown-linux-musl aarch64-unknown-linux-musl)
USER_RUN=("aarch64-apple-darwin (scripts/release.sh on the release Mac: sign, notarise)"
          "airs-mcp container image (deploy/airs-mcp/Dockerfile: build, push, pin the digest)")
host="$(rustc -vV | sed -n 's/^host: //p')"
TARGET_DIR="$(cargo metadata --no-deps --format-version 1 | python3 -I -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')"
installed="$(rustup target list --installed 2>/dev/null || true)"
{
  echo "commit: $(git rev-parse HEAD 2>/dev/null || echo unknown)"
  echo "rustc: $(rustc --version)"
  echo "cargo: $(cargo --version)"
  echo "host: $host"
} > "$OUT/BUILD.txt"
git ls-files -z | LC_ALL=C sort -z | xargs -0 sha256sum > "$OUT/SOURCE-SHA256SUMS"
sums="$OUT/.BIN-SHA256SUMS.partial"
: > "$sums"
fatal=0 missing=()
for t in "${REQUIRED[@]}"; do
  if [ "$t" != "$host" ] && ! grep -qx -- "$t" <<< "$installed"; then
    echo "missing: $t (target not installed here)" >> "$OUT/BUILD.txt"
    missing+=("$t")
    continue
  fi
  if ! cargo build --locked --release --target "$t" -p airs-cli -p airs-mcp >"$OUT/build-$t.log" 2>&1; then
    echo "FAILED: $t (build error; see build-$t.log)" >> "$OUT/BUILD.txt"
    echo "build-hashes: the $t build failed (see $OUT/build-$t.log)" >&2
    fatal=1
    continue
  fi
  for bin in airs airs-mcp; do
    f="$TARGET_DIR/$t/release/$bin"
    if [ ! -f "$f" ]; then
      echo "FAILED: $t ($bin was not produced)" >> "$OUT/BUILD.txt"
      echo "build-hashes: $t built but produced no $bin" >&2
      fatal=1
      continue 2
    fi
    (cd "$TARGET_DIR" && sha256sum "$t/release/$bin") >> "$sums"
  done
  echo "built: $t" >> "$OUT/BUILD.txt"
done
for p in "${USER_RUN[@]}"; do
  echo "user-run: $p (evidence pending)" >> "$OUT/BUILD.txt"
done
pending=("${missing[@]}")
for p in "${USER_RUN[@]}"; do pending+=("${p%% (*}"); done
if [ "$fatal" -ne 0 ]; then
  echo "local-evidence: failed" >> "$OUT/BUILD.txt"
  echo "release-qualification: blocked (a build failed)" >> "$OUT/BUILD.txt"
  rm -f "$sums"
  echo "build-hashes: release qualification blocked: a build failed" >&2
  exit 1
fi
mv "$sums" "$OUT/BIN-SHA256SUMS"
if [ "${#missing[@]}" -gt 0 ]; then
  echo "local-evidence: incomplete" >> "$OUT/BUILD.txt"
else
  echo "local-evidence: complete" >> "$OUT/BUILD.txt"
fi
pending_list="$(IFS=,; echo "${pending[*]}")"
echo "release-qualification: blocked (pending: ${pending_list//,/, })" >> "$OUT/BUILD.txt"
echo "build-hashes: wrote $OUT"
if [ "${#missing[@]}" -gt 0 ]; then
  echo "build-hashes: release qualification blocked: not installed here: ${missing[*]}" >&2
  exit 3
fi
