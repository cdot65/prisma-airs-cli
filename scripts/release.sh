#!/usr/bin/env bash
# Cuts an airs-cli release on the maintainer's Mac. docs/RELEASING.md is the guide.
# usage() below holds the usage text, its options, environment and token handling
# (--help prints it; running with no version prints it on stderr).
#
# Order of the steps with an effect: sign, notarise, create the Forgejo release as a
# draft and upload every asset, push the tag, publish the draft. Every Forgejo write
# that can fail on a missing token scope happens before the tag is public.
#
# The executable is airs; each tarball, airs-cli-<version>-<target>.tar.gz, also holds
# the names airs-cli (from 8.0) and airs-cli-rs (the 1.x compatibility name) as relative
# symlinks to it (DESIGN §21.7, scripts/package-release.sh). The crates.io publish, the
# Homebrew formula and the npm packages follow by hand (docs/RELEASING.md §2, §6 and
# §7); this script does none of them.
#
# -E: the ERR trap also fires inside functions (run, act, check helpers) and
# subshells, so every failure names the step it stopped in.
set -Eeuo pipefail
umask 022

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

API="https://git.cdot.io/api/v1/repos/cdot/prisma-airs-cli"
WEB="https://git.cdot.io/cdot/prisma-airs-cli"
KEYCHAIN_ITEM="airs-cli-forgejo"
SIGN_IDENTITY="${AIRS_SIGN_IDENTITY:-Developer ID Application: Calvin Remsburg (G5QLZ5A8TA)}"
NOTARY_PROFILE="${AIRS_NOTARY_PROFILE:-airs-cli-notary}"
BIN="airs"
ALIAS_BIN="airs-cli"
COMPAT_BIN="airs-cli-rs"
MAC_TARGET="aarch64-apple-darwin"
LINUX_TARGETS=(x86_64-unknown-linux-musl aarch64-unknown-linux-musl)

usage() {
  cat <<'USAGE'
Cuts an airs-cli release on the maintainer's Mac. docs/RELEASING.md is the guide.

  scripts/release.sh <version> [--dry-run] [--skip-notarize]

  <version>        the version in Cargo.toml, without the leading v (e.g. 0.2.0)
  --dry-run        print every step; run only the read-only checks and the builds.
                   Nothing is signed, submitted, tagged or written to Forgejo.
  --skip-notarize  sign the macOS binary but do not notarise it (the release notes
                   then say so)

Environment:
  AIRS_SIGN_IDENTITY    codesign identity (default: Developer ID Application:
                        Calvin Remsburg (G5QLZ5A8TA))
  AIRS_NOTARY_PROFILE   notarytool keychain profile (default: airs-cli-notary)

The Forgejo token is read from the keychain item airs-cli-forgejo when it is needed,
handed to curl on stdin and never printed. The script stops at the first error and
never moves, deletes or re-creates an existing tag.
USAGE
}

version=""
dry_run=0
skip_notarize=0
for arg in "$@"; do
  case "$arg" in
    --dry-run) dry_run=1 ;;
    --skip-notarize) skip_notarize=1 ;;
    -h | --help)
      usage
      exit 0
      ;;
    -*)
      echo "release: unknown option: $arg" >&2
      exit 2
      ;;
    *)
      if [ -n "$version" ]; then
        echo "release: more than one version given" >&2
        exit 2
      fi
      version="$arg"
      ;;
  esac
done
if [ -z "$version" ]; then
  usage >&2
  exit 2
fi
if ! [[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?$ ]]; then
  echo "release: '$version' is not a version like 0.2.0 (no leading v)" >&2
  exit 2
fi
TAG="v$version"
DIST="target/dist/$version"

# --- output helpers ---------------------------------------------------------------

current_step=""
failed_checks=()

step() {
  current_step="$1"
  printf '\n==> %s\n' "$1"
}

die() {
  echo "release: $*" >&2
  exit 1
}

on_error() {
  local status=$?
  # Inside a command substitution or subshell, only leave it; the parent shell reports
  # the failure once.
  if [ "${BASH_SUBSHELL:-0}" -gt 0 ]; then
    exit "$status"
  fi
  echo "release: stopped (exit $status) during: ${current_step:-setup}" >&2
  echo "release: nothing after this step ran; docs/RELEASING.md describes recovery." >&2
  exit "$status"
}
trap on_error ERR

# Print a command exactly as it runs.
show() {
  printf '    $'
  printf ' %q' "$@"
  printf '\n'
}

# A read-only check or a build: runs in every mode.
run() {
  show "$@"
  "$@"
}

# A step with an effect outside target/: printed only in a dry run.
act() {
  if [ "$dry_run" -eq 1 ]; then
    printf '    [dry-run] would run:'
    printf ' %q' "$@"
    printf '\n'
    return 0
  fi
  show "$@"
  "$@"
}

# A precondition. A real run stops at the first failure; a dry run records it, keeps
# going so that every problem is reported, and fails at the end.
check() {
  local what="$1"
  shift
  if "$@"; then
    echo "    ok: $what"
  elif [ "$dry_run" -eq 1 ]; then
    echo "    FAILED: $what"
    failed_checks+=("$what")
  else
    die "check failed: $what"
  fi
}

need() {
  command -v "$1" >/dev/null 2>&1 || die "$1 is required but not on PATH${2:+ ($2)}"
}

# --- Forgejo API ------------------------------------------------------------------

# forgejo <curl args...>: one API call with the keychain token. The token goes to curl
# through a config on stdin, so it is in neither the arguments nor the environment.
# HTTP/1.1: multi-megabyte uploads over HTTP/2 to this Forgejo instance stall until they
# time out (0.3.0 release), while the same upload over HTTP/1.1 takes under a second.
forgejo() {
  local token
  token="$(security find-generic-password -s "$KEYCHAIN_ITEM" -w 2>/dev/null)" \
    || die "cannot read the Forgejo token from keychain item $KEYCHAIN_ITEM"
  [ -n "$token" ] || die "keychain item $KEYCHAIN_ITEM is empty"
  printf 'header = "Authorization: token %s"\n' "$token" | curl -sS --http1.1 -K - "$@"
}

# --- checks -----------------------------------------------------------------------

on_branch_main() { [ "$(git symbolic-ref --quiet --short HEAD 2>/dev/null)" = main ]; }
tree_is_clean() { [ -z "$(git status --porcelain --untracked-files=normal)" ]; }
head_is_origin_main() {
  local remote
  remote="$(git ls-remote origin refs/heads/main | cut -f1)"
  [ -n "$remote" ] && [ "$remote" = "$(git rev-parse HEAD)" ]
}
cargo_version() {
  cargo metadata --no-deps --format-version 1 --locked \
    | jq -r '.packages[] | select(.name == "airs-cli") | .version'
}
cargo_version_matches() { [ "$(cargo_version)" = "$version" ]; }
tag_absent_locally() { ! git rev-parse --quiet --verify "refs/tags/$TAG" >/dev/null; }
tag_absent_on_origin() {
  local refs
  refs="$(git ls-remote --tags origin "refs/tags/$TAG")" || return 1
  [ -z "$refs" ]
}
release_absent_on_forgejo() {
  local code
  code="$(forgejo -o /dev/null -w '%{http_code}' "$API/releases/tags/$TAG")" || return 1
  [ "$code" = 404 ]
}
# A draft left by an earlier run is not found by tag, so list the drafts too.
no_draft_for_tag() {
  local drafts
  drafts="$(forgejo -f "$API/releases?draft=true&limit=50")" || return 1
  jq -e --arg tag "$TAG" 'type == "array" and all(.[]; .tag_name != $tag)' <<<"$drafts" >/dev/null
}
# The token's user can push to the repository. This does not prove the token's scope
# (a read-only token of an admin passes); the first scoped write is the draft release,
# which runs before the tag is pushed.
token_can_push() {
  local repo
  repo="$(forgejo -f "$API")" || return 1
  jq -e '.permissions.push == true' <<<"$repo" >/dev/null
}
sign_identity_present() {
  security find-identity -v -p codesigning | grep -F -q "\"$SIGN_IDENTITY\""
}
reference_is_current() { scripts/build-reference-cli.sh --verify >/dev/null; }

# --- start ------------------------------------------------------------------------

if [ "$dry_run" -eq 1 ]; then
  echo "release $TAG (DRY RUN: checks and builds only; nothing is signed, tagged or published)"
else
  echo "release $TAG"
fi

step "Tools"
[ "$(uname -s)" = Darwin ] || die "releases are cut on macOS (signing and notarisation need it)"
[ "$(uname -m)" = arm64 ] || die "the macOS build is native $MAC_TARGET; run this on Apple Silicon"
for tool in git cargo rustup jq curl shasum tar security codesign xcrun ditto spctl file node; do
  need "$tool"
done
need cargo-zigbuild "cargo install --locked cargo-zigbuild"
need zig "brew install zig"
installed_targets="$(rustup target list --installed)"
for target in "$MAC_TARGET" "${LINUX_TARGETS[@]}"; do
  grep -qx "$target" <<<"$installed_targets" || die "rust target $target is missing (rustup target add $target)"
done
tar --version 2>/dev/null | head -1 | grep -q '^bsdtar ' \
  || die "tar is not bsdtar (the packaging needs --no-mac-metadata and stores the $ALIAS_BIN and $COMPAT_BIN links)"
echo "    $(cargo --version)"
echo "    zig $(zig version), $(cargo-zigbuild --version)"

step "Repository state"
check "on branch main" on_branch_main
check "working tree is clean (no changes, no untracked files)" tree_is_clean
check "HEAD is origin/main" head_is_origin_main
check "Cargo.toml version is $version (found $(cargo_version))" cargo_version_matches
check "tag $TAG does not exist locally" tag_absent_locally
check "tag $TAG does not exist on origin" tag_absent_on_origin
check "no Forgejo release for $TAG" release_absent_on_forgejo
check "no Forgejo draft release for $TAG" no_draft_for_tag
check "the Forgejo token's user can push to the repository" token_can_push
check "codesign identity \"$SIGN_IDENTITY\" is in the keychain" sign_identity_present
if [ -f "docs/RELEASE-NOTES/$version.md" ]; then
  echo "    notes: docs/RELEASE-NOTES/$version.md"
else
  echo "    notes: docs/RELEASE-NOTES/$version.md is absent; the release gets a default body"
fi

step "Required checks (fmt, clippy, tests with the TypeScript reference)"
check "TypeScript reference build is current (scripts/build-reference-cli.sh --verify)" reference_is_current
run cargo fmt --all --check
run cargo clippy --workspace --all-targets --locked -- -D warnings
# The live tier never runs as part of a release.
run env -u AIRS_RS_LIVE -u AIRS_E2E_TENANT -u AIRS_E2E_PROFILE -u AIRS_ALLOW_PARITY_SKIP \
  AIRS_REQUIRE_PARITY=1 cargo test --workspace --locked --no-fail-fast

step "Build $MAC_TARGET"
# --bin: only airs is built for a release; the other names are links in the tarball,
# so their binaries can never be packaged (DESIGN §21.7).
run cargo build --release --locked --target "$MAC_TARGET" --bin "$BIN"
step "Build ${LINUX_TARGETS[*]} (cargo-zigbuild)"
for target in "${LINUX_TARGETS[@]}"; do
  run cargo zigbuild --release --locked --target "$target" --bin "$BIN"
done

step "Check the binaries"
mac_bin="target/$MAC_TARGET/release/$BIN"
for target in "$MAC_TARGET" "${LINUX_TARGETS[@]}"; do
  bin="target/$target/release/$BIN"
  [ -x "$bin" ] || die "missing $bin"
  description="$(file -b "$bin")"
  echo "    $target: $description"
  case "$target" in
    aarch64-apple-darwin) [[ "$description" == *"Mach-O 64-bit executable arm64"* ]] ;;
    x86_64-unknown-linux-musl) [[ "$description" == *"ELF 64-bit"*"x86-64"* ]] ;;
    aarch64-unknown-linux-musl) [[ "$description" == *"ELF 64-bit"*"ARM aarch64"* ]] ;;
  esac || die "$bin is not a $target executable"
done
got="$("$mac_bin" --version)"
[ "$got" = "$version" ] || die "$mac_bin --version prints '$got', expected '$version'"
echo "    $mac_bin --version: $got"

step "Sign the macOS binary"
act codesign --force --options runtime --timestamp --sign "$SIGN_IDENTITY" "$mac_bin"
if [ "$dry_run" -eq 0 ]; then
  run codesign --verify --strict --verbose=2 "$mac_bin"
  details="$(codesign -dvv "$mac_bin" 2>&1)"
  grep -F -q "Authority=$SIGN_IDENTITY" <<<"$details" || die "signature authority is not $SIGN_IDENTITY"
  grep -q '^Timestamp=' <<<"$details" || die "signature has no secure timestamp"
  grep -q 'flags=.*runtime' <<<"$details" || die "signature lacks the hardened runtime"
  grep -qx "Identifier=$BIN" <<<"$details" || die "signature identifier is not $BIN"
  echo "    signature: Developer ID, hardened runtime, secure timestamp, identifier $BIN"
else
  echo "    [dry-run] would verify: codesign --verify --strict; Authority, Timestamp, runtime flag and Identifier=$BIN"
fi

notarized=0
step "Notarise the macOS binary"
if [ "$skip_notarize" -eq 1 ]; then
  echo "    skipped (--skip-notarize): the binary is signed but not notarised"
else
  notary_zip="target/dist/notarize-$version-$MAC_TARGET.zip"
  if [ "$dry_run" -eq 1 ]; then
    echo "    [dry-run] would run: ditto -c -k --keepParent $mac_bin $notary_zip"
    echo "    [dry-run] would run: xcrun notarytool submit $notary_zip --keychain-profile $NOTARY_PROFILE --wait --output-format json"
  else
    mkdir -p target/dist
    rm -f "$notary_zip"
    run ditto -c -k --keepParent "$mac_bin" "$notary_zip"
    show xcrun notarytool submit "$notary_zip" --keychain-profile "$NOTARY_PROFILE" --wait --output-format json
    # A non-zero exit (Invalid on some Xcode versions, authentication, a network drop
    # during --wait, an interrupt) must not skip the id and the log, so the status is
    # captured instead of tripping errexit. The output may not be JSON then.
    notary_rc=0
    result="$(xcrun notarytool submit "$notary_zip" --keychain-profile "$NOTARY_PROFILE" --wait --output-format json)" || notary_rc=$?
    submission="$(jq -r '.id // empty' <<<"$result" 2>/dev/null)" || submission=""
    status="$(jq -r '.status // empty' <<<"$result" 2>/dev/null)" || status=""
    echo "    submission ${submission:-?}: ${status:-unknown} (notarytool exit $notary_rc)"
    if [ "$notary_rc" -ne 0 ] || [ "$status" != Accepted ]; then
      if [ -n "$submission" ]; then
        xcrun notarytool log "$submission" --keychain-profile "$NOTARY_PROFILE" >&2 || true
        die "notarisation was not accepted (submission $submission); look it up with: xcrun notarytool info $submission --keychain-profile $NOTARY_PROFILE; see docs/RELEASING.md (Recovery)"
      fi
      [ -z "$result" ] || printf '%s\n' "$result" >&2
      die "notarisation failed before a submission id was returned; see docs/RELEASING.md (Recovery)"
    fi
    rm -f "$notary_zip"
    notarized=1
  fi
  # A bare Mach-O binary cannot be stapled: Gatekeeper fetches its notarisation ticket
  # online. spctl assesses apps, so for a command-line tool it reports "does not seem to
  # be an app" even when signed and notarised; its verdict is shown, not enforced.
  echo "    note: bare binaries cannot be stapled; the ticket is looked up online"
fi
if [ "$dry_run" -eq 0 ]; then
  echo "    spctl (informational): $({ spctl --assess --type execute -vv "$mac_bin" 2>&1 || true; } | tr '\n' ' ')"
fi

step "Package into $DIST"
run rm -rf "$DIST"
run mkdir -p "$DIST"
# scripts/package-release.sh stages airs, the airs-cli and airs-cli-rs links, README.md,
# LICENSE and NOTICE per target, writes the tarballs and SHA256SUMS, then lists and
# extracts each tarball to check its entries, links, modes and owner, and runs the three
# names' --version on macOS.
package_args=(--run-check "$MAC_TARGET")
if [ "$dry_run" -eq 0 ]; then
  # The signature must survive packaging: codesign --verify on the extracted airs.
  package_args+=(--verify-signature "$MAC_TARGET")
fi
package_specs=()
for target in "$MAC_TARGET" "${LINUX_TARGETS[@]}"; do
  package_specs+=("$target=target/$target/release/$BIN")
done
run scripts/package-release.sh "${package_args[@]}" "$version" "$DIST" "${package_specs[@]}"
# The tarball names are package-release.sh's alone: its SHA256SUMS lists exactly the
# tarballs it wrote and checked, one "<sha256>  <name>" line each, in target order.
tarballs=()
while IFS= read -r line; do
  name="${line#*  }"
  [[ "$name" =~ ^[A-Za-z0-9._-]+\.tar\.gz$ ]] || die "$DIST/SHA256SUMS has an unexpected line: $line"
  tarballs+=("$name")
done <"$DIST/SHA256SUMS"
[ "${#tarballs[@]}" -eq "${#package_specs[@]}" ] \
  || die "$DIST/SHA256SUMS lists ${#tarballs[@]} tarballs, expected ${#package_specs[@]}"
if [ "$dry_run" -eq 1 ]; then
  echo "    [dry-run] would verify: codesign --verify --strict on the packaged $BIN ($MAC_TARGET)"
  printf 'dry run: the macOS binary in this package is NOT signed or notarised\n' > "$DIST/DRY-RUN"
fi
(cd "$DIST" && sed 's/^/    /' SHA256SUMS)

if [ "$dry_run" -eq 1 ] && [ "${#failed_checks[@]}" -gt 0 ]; then
  step "Publish (not reached)"
  echo "    a real run would have stopped at the first failed check"
fi

step "Forgejo draft release $TAG"
notes_file="docs/RELEASE-NOTES/$version.md"
body_file="$(mktemp "${TMPDIR:-/tmp}/airs-release-notes.XXXXXX")"
payload_file="$(mktemp "${TMPDIR:-/tmp}/airs-release-payload.XXXXXX")"
trap 'rm -f "$body_file" "$payload_file"' EXIT
if [ -f "$notes_file" ]; then
  cat "$notes_file" > "$body_file"
else
  printf 'airs-cli %s.\n\nSee docs/COMPATIBILITY.md and docs/DESIGN.md section 12 for the differences from the TypeScript CLI.\n' "$version" > "$body_file"
fi
if [ "$skip_notarize" -eq 1 ]; then
  printf '\n> The macOS binary in this release is signed but not notarised.\n' >> "$body_file"
fi
printf '\nVerify a download with `shasum -a 256 -c SHA256SUMS --ignore-missing`.\n' >> "$body_file"
prerelease=false
[[ "$version" == *-* ]] && prerelease=true
payload="$(jq -n --arg tag "$TAG" --arg commit "$(git rev-parse HEAD)" \
  --arg name "airs-cli $version" --rawfile body "$body_file" --argjson pre "$prerelease" \
  '{tag_name: $tag, target_commitish: $commit, name: $name, body: $body, draft: true, prerelease: $pre}')"
release_id=""
if [ "$dry_run" -eq 1 ]; then
  echo "    [dry-run] would POST $API/releases (draft; Forgejo creates no tag for a draft) with:"
  jq '{tag_name, target_commitish, name, draft, prerelease, body_bytes: (.body | length)}' <<<"$payload" | sed 's/^/      /'
  for asset in "${tarballs[@]}" SHA256SUMS; do
    echo "    [dry-run] would POST $API/releases/<id>/assets?name=$asset"
  done
else
  # A draft creates no tag and is not public, so a token without write scope or a
  # failed upload stops here with nothing published.
  # stdin carries the token config, so the payload goes through a file.
  printf '%s' "$payload" > "$payload_file"
  response="$(forgejo -X POST -H 'Content-Type: application/json' --data-binary "@$payload_file" \
    -w '\n%{http_code}' "$API/releases")"
  code="${response##*$'\n'}"
  [ "$code" = 201 ] || die "creating the draft release failed (HTTP $code); nothing was tagged or published"
  release_id="$(jq -r '.id' <<<"${response%$'\n'*}")"
  [[ "$release_id" =~ ^[0-9]+$ ]] || die "the release response has no id"
  echo "    draft release $release_id created"
  for asset in "${tarballs[@]}" SHA256SUMS; do
    # Up to three attempts. A timed-out attempt may still have stored the asset, so
    # before retrying, an asset already on the draft with the local size counts as
    # uploaded (a second POST would add a duplicate).
    size="$(wc -c <"$DIST/$asset" | tr -d ' ')"
    uploaded=0
    for attempt in 1 2 3; do
      code="$(forgejo -X POST --max-time 300 -o /dev/null -w '%{http_code}' \
        -F "attachment=@$DIST/$asset" "$API/releases/$release_id/assets?name=$asset")" || code="${code:-000}"
      if [ "$code" = 201 ]; then uploaded=1; break; fi
      echo "    upload of $asset failed (attempt $attempt, HTTP $code)"
      if forgejo "$API/releases/$release_id/assets" \
        | jq -e --arg n "$asset" --argjson s "$size" 'any(.[]; .name == $n and .size == $s)' >/dev/null; then
        uploaded=1; break
      fi
    done
    [ "$uploaded" = 1 ] || die "uploading $asset failed (HTTP $code); draft release $release_id is unpublished and nothing was tagged"
    echo "    uploaded $asset"
  done
  count="$(forgejo "$API/releases/$release_id/assets" | jq 'length')"
  [ "$count" = "$(( ${#tarballs[@]} + 1 ))" ] || die "draft release $release_id has $count assets, expected $(( ${#tarballs[@]} + 1 )); nothing was tagged"
fi

step "Tag $TAG"
act git tag -a "$TAG" -m "airs-cli $version"
if [ "$dry_run" -eq 0 ]; then
  show git push origin "refs/tags/$TAG"
  if ! git push origin "refs/tags/$TAG"; then
    # The tag was made by this run and never published, so removing it is safe.
    git tag -d "$TAG" >/dev/null
    die "pushing $TAG failed; the local tag was removed; draft release $release_id is unpublished"
  fi
else
  echo "    [dry-run] would run: git push origin refs/tags/$TAG"
fi

step "Publish Forgejo release $TAG"
if [ "$dry_run" -eq 1 ]; then
  echo "    [dry-run] would PATCH $API/releases/<id> {\"draft\": false}"
else
  code="$(forgejo -X PATCH -H 'Content-Type: application/json' -o /dev/null -w '%{http_code}' \
    --data-binary '{"draft": false}' "$API/releases/$release_id")"
  [ "$code" = 200 ] || die "publishing draft release $release_id failed (HTTP $code); $TAG is pushed"
  echo "    published: $WEB/releases/tag/$TAG"
fi

echo
if [ "$dry_run" -eq 1 ]; then
  if [ "${#failed_checks[@]}" -gt 0 ]; then
    echo "dry run finished with ${#failed_checks[@]} failed check(s):"
    printf '  - %s\n' "${failed_checks[@]}"
    exit 1
  fi
  echo "dry run finished: every check passed and every build succeeded"
else
  if [ "$skip_notarize" -eq 0 ] && [ "$notarized" -ne 1 ]; then
    die "internal error: the release finished without notarisation"
  fi
  echo "release $TAG finished"
  if [ "$prerelease" = true ]; then
    echo "next: cargo publish the two library crates, then airs-cli, in the order docs/RELEASING.md §2 gives (a pre-release gets no Homebrew formula)"
  else
    echo "next: cargo publish the two library crates, then airs-cli (docs/RELEASING.md §2 order); then scripts/homebrew-formula.sh $version (§6) and scripts/npm-package.sh $version (§7)"
  fi
fi
