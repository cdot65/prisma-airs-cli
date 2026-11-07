#!/usr/bin/env bash
# Build a frozen pre-M7 `airs` (the 8.0.0 release commit) for the M7 refusal test
# (RUNTIME_MCP_CONTRACT §7.3): older binaries must refuse a tenant file with an `mcp`
# table. Read-only on the repository: `git archive` of the commit into a scratch
# directory, built there. Prints the binary's path; then run
#   AIRS_PRE_M7_BIN=<path> cargo test --test mcp_client a_frozen_pre_m7_binary
set -euo pipefail
repo=${AIRS_PRE_M7_REPO:-$(git rev-parse --show-toplevel)}
ref=${AIRS_PRE_M7_REF:-f3e61ac}   # release: airs-cli 8.0.0
out=${AIRS_PRE_M7_DIR:-${TMPDIR:-/tmp}/airs-pre-m7-$ref}
mkdir -p "$out/src"
git -C "$repo" archive "$ref" | tar -x -C "$out/src"
(cd "$out/src" && CARGO_TARGET_DIR="$out/target" cargo build --locked --release --bin airs >&2)
echo "$out/target/release/airs"
