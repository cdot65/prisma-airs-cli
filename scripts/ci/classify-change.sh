#!/usr/bin/env bash
# classify-change.sh BASE: print `rust=false` when every path that differs between the merge
# base of BASE and HEAD is documentation (docs/, docs-site/, or any *.md), else `rust=true`.
# CI uses it to skip the Rust steps for docs-only changes. It fails safe: a missing, all-zero
# or unknown BASE, no common ancestor, a failing git command or an empty diff all print
# `rust=true`. Renames are disabled, so a moved file contributes its old path as a deletion;
# paths are read NUL-separated and every path is examined (no early-exit pipeline).
set -u
base="${1:-}"
safe() { echo "rust=true"; exit 0; }
[ -n "$base" ] || safe
[ "$base" != 0000000000000000000000000000000000000000 ] || safe
git cat-file -e "$base^{commit}" 2>/dev/null || safe
merge_base="$(git merge-base "$base" HEAD 2>/dev/null)" || safe
[ -n "$merge_base" ] || safe
list="$(mktemp)"
trap 'rm -f "$list"' EXIT
git diff --no-renames --name-only -z "$merge_base" HEAD > "$list" 2>/dev/null || safe
[ -s "$list" ] || safe
docs_only=true
while IFS= read -r -d '' path; do
  case "$path" in
    docs/* | docs-site/* | *.md) ;;
    *) docs_only=false ;;
  esac
done < "$list"
if [ "$docs_only" = true ]; then echo "rust=false"; else echo "rust=true"; fi
