# Releasing airs-cli

Releases are cut by the maintainer on an Apple Silicon Mac, from `cdot/prisma-airs-cli`
(the Rust CLI moved here from `cdot/prisma-airs-cli-rs` for 8.0). From 8.0.0 one version
is used everywhere: the crate, `airs --version`, the tag, the Forgejo release, the Homebrew
formula and npm. A release has four parts, in this order:

1. **The Forgejo release** (`scripts/release.sh`): builds the `airs` executable for
   three targets, signs and notarises the macOS one, packages each with the `airs-cli`
   and `airs-cli-rs` links and `NOTICE` (`scripts/package-release.sh`), uploads everything
   to a draft Forgejo release, pushes the tag and publishes the draft.
2. **The crates.io publish** (§2 step 7), from the tagged commit, in dependency order:
   `airs-runtime-core`, then `airs-mcp-client`, then `airs-cli` (the CLI pins both at
   its exact version). A published version can only be yanked, so it follows a release
   that succeeded.
3. **The Homebrew formula** (§6): rendered from the published crate and committed to
   the tap. It builds from the crates.io crate and needs its checksum.
4. **The npm packages** (§7): staged from the release tarballs by
   `scripts/npm-package.sh`, then published, platform versions first.

Signing moves to the `jadzia` runner later ([MILESTONES.md](MILESTONES.md),
"Dependencies and releases"); that runner must first meet the self-hosted runner
requirements in [TESTING.md](TESTING.md) §10, and signing then gets its own runner
registration, user and label that pull requests never reach.

```sh
scripts/release.sh <version> [--dry-run] [--skip-notarize]
```

`<version>` is the version in `Cargo.toml`, without the leading `v`. The tag is
`v<version>`.

## 1. Prerequisites

One-time setup on the release Mac:

| What | How to check | How to set up |
|---|---|---|
| Apple Silicon macOS | `uname -sm` prints `Darwin arm64` | |
| Rust and the targets | `rustup target list --installed` lists `aarch64-apple-darwin`, `x86_64-unknown-linux-musl`, `aarch64-unknown-linux-musl` | `rustup target add x86_64-unknown-linux-musl aarch64-unknown-linux-musl` |
| zig and cargo-zigbuild | `zig version`, `cargo-zigbuild --version` | `brew install zig`, `cargo install --locked cargo-zigbuild` |
| Node 24 and the TypeScript reference | `scripts/build-reference-cli.sh --verify` | `scripts/build-reference-cli.sh` ([TESTING.md](TESTING.md) §3) |
| `jq` | `jq --version` | `brew install jq` (macOS 15 and later ship `/usr/bin/jq`) |
| bsdtar | `tar --version` starts `bsdtar` | the macOS `/usr/bin/tar`; the packaging stores the `airs-cli` and `airs-cli-rs` links and drops macOS metadata with them |
| Developer ID certificate | `security find-identity -v -p codesigning` lists `Developer ID Application: Calvin Remsburg (G5QLZ5A8TA)` | Install the certificate and its private key in the login keychain. `AIRS_SIGN_IDENTITY` overrides the identity. |
| Notary credentials | `xcrun notarytool history --keychain-profile airs-cli-notary` | `xcrun notarytool store-credentials airs-cli-notary --apple-id <id> --team-id G5QLZ5A8TA` (it prompts for an app-specific password). `AIRS_NOTARY_PROFILE` overrides the profile name. |
| Forgejo token | `security find-generic-password -s airs-cli-forgejo >/dev/null` (without `-w`, so the token is not shown) | Create a token with the `write:repository` scope, then `security add-generic-password -s airs-cli-forgejo -a <user> -w` (it prompts for the token). A token without write scope fails at the draft release, before the tag is pushed. |
| ssh push access to `origin` | `git ls-remote origin` | |
| crates.io login | `cargo publish --dry-run -p airs-cli` gets past the upload check | `cargo login` |
| npm login (§7) | `npm whoami` prints the publishing user (without printing a token) | the publishing token is held at Conjur `data/airs-release-bot/npmjs/token` (FORGEJO.md); put it in a private npm user config (`npm_config_userconfig` pointing at a 0600 file), never on a command line or in the repository |
| Homebrew and the tap (§6) | `brew --repository cdot65/prisma-airs-cli` prints a path | `brew tap cdot65/prisma-airs-cli`, `brew trust cdot65/prisma-airs-cli` and `gh auth setup-git` (push access to the GitHub tap repository) |

Never pass the token or the app-specific password on a command line, and never run the
script under `set -x` or `bash -x`.

## 2. Cutting a release

1. Bump `version` in `Cargo.toml`, run `cargo build` so `Cargo.lock` follows, and
   update the version shown in `README.md`.
2. Write `docs/RELEASE-NOTES/<version>.md`. List the new DESIGN §12 rows and any
   change to the command surface. Without the file the release gets a short default
   body.
3. Commit and push to `main`. Wait for the CI `linux` and `musl` jobs to pass
   ([TESTING.md](TESTING.md) §10).
4. Run `scripts/release.sh <version> --dry-run` and read the output. It must end with
   `dry run finished: every check passed and every build succeeded`.
5. Run `scripts/release.sh <version>`. Notarisation usually takes a few minutes.
6. Check the release page and download one tarball to verify it
   (§4).
7. Publish the crates from the tagged commit, on a clean tree, in dependency order:
   `cargo publish -p airs-runtime-core`, then `cargo publish -p airs-mcp-client`, then
   `cargo publish --dry-run -p airs-cli` and `cargo publish -p airs-cli`. Each must be
   visible on crates.io before the next resolves it. It needs a
   crates.io login (`cargo login`). A published version can only be yanked, never
   replaced, so publish after the Forgejo release succeeded. If publishing fails after
   the tag exists, fix the cause and release the next patch version. The script's last
   line names this step and the next.
8. Render the Homebrew formula from the published crate, check it and commit it to the
   tap (§6). A pre-release (`1.1.0-rc.1`) gets no formula: the tap carries stable
   releases only.
9. Stage and publish the npm packages from the release tarballs (§7).

## 3. What the script does

Each step is printed before it runs. The script stops at the first error.

| Step | Real run | `--dry-run` |
|---|---|---|
| Tools: macOS arm64, the tools above (`tar` must be bsdtar), the three rust targets | runs | runs |
| Repository state: on `main`, clean tree with no untracked files, `HEAD` equal to `origin/main`, `Cargo.toml` version equal to `<version>`, no `v<version>` tag locally or on `origin`, no Forgejo release or draft release for the tag, the token's user can push to the repository, codesign identity present | stops at the first failure | runs every check, reports each failure, exits 1 at the end |
| Checks: reference `--verify`, `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `AIRS_REQUIRE_PARITY=1 cargo test --workspace --no-fail-fast` (live variables removed from the environment) | runs | runs |
| Builds: `cargo build --release --bin airs` for `aarch64-apple-darwin`, `cargo zigbuild --release --bin airs` for both musl targets (only `airs` is built: the compatibility name is a link in the tarball); checks the file type of each `target/<target>/release/airs` and the macOS `--version` | runs | runs |
| Sign `airs`: `codesign --force --options runtime --timestamp --sign <identity>`, then `codesign --verify --strict` and a check for the Developer ID authority, the secure timestamp, the hardened runtime and `Identifier=airs` | runs | printed only |
| Notarise: `ditto -c -k --keepParent` of `airs` into a zip, `xcrun notarytool submit --keychain-profile airs-cli-notary --wait`; a non-zero exit or anything but `Accepted` prints the submission id and the notary log (when notarytool returned an id) and stops | runs (skipped by `--skip-notarize`) | printed only |
| Package (`scripts/package-release.sh`): `target/dist/<version>/airs-cli-<version>-<target>.tar.gz`, each holding exactly `airs` (0755), `airs-cli` and `airs-cli-rs` (relative symlinks to `airs`), `README.md`, `LICENSE` and `NOTICE` (0644), root-owned, and `SHA256SUMS` of the three tarballs. Each tarball is then listed and extracted into `.verify/` to check the entries and their order, the link targets, the modes, the owner and that the files equal their sources; the macOS `./airs --version`, `./airs-cli --version` and `./airs-cli-rs --version` must print the version | runs; then `codesign --verify --strict` on the extracted macOS `airs` (the signature survived packaging) | runs, without the signature check (printed); the directory gets a `DRY-RUN` marker because its macOS binary is unsigned |
| Draft release: create a draft release on Forgejo with the notes (a draft creates no tag and is not public), upload the three tarballs and `SHA256SUMS` (up to three attempts each; an asset already on the draft with the local size counts as uploaded, so a retry never duplicates it), check the asset count | runs | printed only |
| Tag: `git tag -a v<version>` and `git push origin refs/tags/v<version>` | runs | printed only |
| Publish: `PATCH` the draft to `draft: false`; the release is named `airs-cli <version>` (so is the tag message), and the last line names the next steps (`cargo publish -p airs-cli`, then `scripts/homebrew-formula.sh <version>`) | runs | printed only |

Notes:

- A bare Mach-O binary cannot be stapled. Gatekeeper looks the notarisation ticket up
  online the first time the binary runs. `spctl --assess --type execute` assesses apps,
  so for a command-line tool it prints `does not seem to be an app` even when the
  binary is signed and notarised. The script shows its verdict but does not act on it;
  `codesign --verify --strict` and the `Accepted` notary status are the checks that
  count.
- `--skip-notarize` signs the binary and publishes it without notarisation. The release
  notes then say so. Use it only when Apple's service is down and the release cannot
  wait.
- The release is created as a draft and every asset is uploaded before the tag is
  pushed, so a token without write scope or a failed upload stops the run with no tag
  and no public release. The draft is published only after the tag is pushed.
- Every failure prints `release: stopped (exit N) during: <step>` or a `release:` line
  naming the cause (the script runs under `set -E`, so the trap also covers commands
  run inside its helper functions).
- Every API call uses HTTP/1.1. Multi-megabyte uploads over HTTP/2 to this Forgejo
  instance stalled until they timed out during the 0.3.0 release.
- A version with a pre-release suffix (`0.2.0-rc.1`) is published as a Forgejo
  pre-release.
- `scripts/package-release.sh` can be run on its own (`--help` prints its usage). It
  refuses an empty argument, a dist directory that exists and is not empty, a missing
  or non-executable binary and a tar that is not bsdtar before writing anything, and a
  failed check removes everything it wrote. `tests/release_packaging.rs` runs it on
  stub binaries (and, for `--verify-signature`, on a copy of the Apple-signed
  `/usr/bin/true`).
- release.sh takes the asset names from the `SHA256SUMS` that package-release.sh wrote,
  so the tarball naming rule is in package-release.sh alone.
- The script never renders, commits or pushes the Homebrew formula: when it runs, the
  crate is not on crates.io yet.

## 4. Verifying a release

```sh
shasum -a 256 -c SHA256SUMS --ignore-missing
tar -tvzf airs-cli-<version>-aarch64-apple-darwin.tar.gz   # airs, airs-cli -> airs, airs-cli-rs -> airs, README.md, LICENSE, NOTICE
tar -xzf airs-cli-<version>-aarch64-apple-darwin.tar.gz
codesign --verify --strict --verbose=2 airs
codesign -dvv airs 2>&1 | grep -E '^(Identifier|Authority|TeamIdentifier|Timestamp)='
./airs --version
./airs-cli --version
./airs-cli-rs --version
```

All three print the bare version. `Identifier=airs` is the signed file's name.

## 5. Recovery

The script never moves, deletes or re-creates an existing tag. Where it stopped decides
what to do.

**A check or a build failed.** Nothing was signed, tagged or published. Fix the cause,
commit and push to `main` if code changed, and run the script again.

**Signing failed.** Usually the certificate or its private key is missing, or the
keychain is locked. Check `security find-identity -v -p codesigning`, unlock the login
keychain, and run again. Nothing was tagged or published.

**Notarisation failed.** Nothing was tagged or published. When notarytool returned a
submission id, the script prints it, the notary log and the `notarytool info` command,
whatever notarytool's exit code. Without an id (authentication failed before
submitting, for example) it prints notarytool's output instead.

- `Invalid`: the log names the problem (most often a missing hardened runtime or secure
  timestamp, which the script sets, or a binary signed with another identity). Fix it
  and run again; the next run rebuilds and re-signs from scratch.
- Authentication errors: the `airs-cli-notary` profile is missing or its app-specific
  password was revoked. Store it again with `xcrun notarytool store-credentials`.
- The service is down or `--wait` timed out: look the submission up with
  `xcrun notarytool info <id> --keychain-profile airs-cli-notary`. When it is
  `Accepted` later, run the script again (it re-submits the rebuilt binary). If the
  release cannot wait, run with `--skip-notarize` and say so in the notes.

**Creating the draft or uploading an asset failed.** Nothing was tagged or published.
Most often the token lacks the `write:repository` scope (HTTP 403) or has expired
(HTTP 401); replace it in the keychain (§1). A draft the script created is left
unpublished, and the next run refuses to start while it exists ("no Forgejo draft
release"): delete it (below), then run again.

**Pushing the tag failed.** The script removes the local tag it just made (it was never
published), so the next run starts clean once the draft is deleted.

**The tag is pushed but publishing failed.** The tag now exists, so the script refuses
to run for this version again. The draft holds every asset; publish it by hand (below),
or bump to the next patch version and release that.

The commands below hand the token to curl through a config on stdin, as the script
does: `printf` is a shell builtin, so the token never appears in a process's arguments
and is never printed.

```sh
api=https://git.cdot.io/api/v1/repos/cdot/prisma-airs-cli
fj() {
  printf 'header = "Authorization: token %s"\n' \
    "$(security find-generic-password -s airs-cli-forgejo -w)" | curl -sS -K - "$@"
}
# The draft release, if the script created one:
fj "$api/releases?draft=true" | jq '.[] | select(.draft) | {id, tag_name, draft}'
# Upload a missing asset (repeat per file):
fj -o /dev/null -w '%{http_code}\n' \
  -F "attachment=@target/dist/<version>/SHA256SUMS" "$api/releases/<id>/assets?name=SHA256SUMS"
# Publish the draft (only once the tag is pushed):
fj -o /dev/null -w '%{http_code}\n' -X PATCH -H 'Content-Type: application/json' \
  --data '{"draft": false}' "$api/releases/<id>"
# Delete an unpublished draft before re-running the script:
fj -o /dev/null -w '%{http_code}\n' -X DELETE "$api/releases/<id>"
```

If the tag is pushed and no draft exists, create one with `fj -X POST -H
'Content-Type: application/json' --data @payload.json "$api/releases"`, where
`payload.json` holds `{"tag_name": "v<version>", "name": "airs-cli <version>",
"body": "...", "draft": true}`, upload the assets and publish it.

**The crates.io publish failed.** The Forgejo release stands. Fix the cause (a missing
login, a network failure) and publish again from the tagged commit; if the cause is in
the crate itself, release the next patch version. Do not render a formula until the
crate is published: the formula script refuses a version crates.io does not have.

**The formula is wrong or does not build.** The tap is separate from the release: fix
the template or the script on `main`, render again and commit to the tap. A yanked
version is refused by the script; render the newest version that is not yanked.

## 6. The Homebrew tap

The Forgejo repository and its release assets need a login, so the formula builds from
the public crates.io crate (DESIGN §21.8). The tap is a separate public GitHub repository,
`cdot65/homebrew-prisma-airs-cli`, holding `Formula/airs-cli.rb` and a README. Homebrew
maps `brew tap cdot65/prisma-airs-cli` to a repository named `homebrew-prisma-airs-cli`.
It replaced the Forgejo `cdot/homebrew-tap`, which needed ssh access to tap.

**One-time setup.** The repository exists. Tap and trust it on the release Mac, and let
git push to GitHub with the `gh` login:

```sh
brew tap cdot65/prisma-airs-cli
brew trust cdot65/prisma-airs-cli
gh auth setup-git
```

The tap's clone, `$(brew --repository cdot65/prisma-airs-cli)`, is the checkout the
formula is committed from.

**For each stable release**, after `cargo publish -p airs-cli` succeeded:

```sh
scripts/homebrew-formula.sh <version>
tap="$(brew --repository cdot65/prisma-airs-cli)"
git -C "$tap" pull --ff-only
cp target/homebrew/Formula/airs-cli.rb "$tap/Formula/airs-cli.rb"
# Until the commit below the formula is an uncommitted change in the tap, and Homebrew's
# auto-update (run by brew install and brew upgrade) stashes such changes.
export HOMEBREW_NO_AUTO_UPDATE=1
brew install --build-from-source cdot65/prisma-airs-cli/airs-cli    # or: brew upgrade --build-from-source airs-cli
brew test cdot65/prisma-airs-cli/airs-cli
brew audit --strict cdot65/prisma-airs-cli/airs-cli
git -C "$tap" diff --stat                              # Formula/airs-cli.rb, and nothing else
git -C "$tap" add Formula/airs-cli.rb
git -C "$tap" commit -m "airs-cli <version>"
git -C "$tap" push
unset HOMEBREW_NO_AUTO_UPDATE
```

- `scripts/homebrew-formula.sh <version>` asks crates.io's public API for the version
  (no credential; `curl -q`, so no `.curlrc` is read, with https only and the user agent
  `airs-cli-homebrew-formula (+https://git.cdot.io/cdot/prisma-airs-cli)`), refuses
  a version that is missing, yanked or has no checksum, downloads the `.crate` file and
  requires its SHA-256 to equal the API's, then renders
  `packaging/homebrew/airs-cli.rb.in` into `target/homebrew/Formula/airs-cli.rb`
  (replacing an earlier render whole). It refuses a pre-release. `--checksum <sha256>`
  or `--metadata-file <saved answer>` render without the network, and `--output <path>`
  or `--output -` write elsewhere; `--help` prints the usage.
- `brew test` runs the formula's `test do` block: `airs --version` and the `airs-cli`
  and `airs-cli-rs` links' `--version` print the version, and `airs tenant list --output
  json` on an empty store prints `[]` (no request is sent).
- `brew audit --strict` must report nothing.
- `HOMEBREW_NO_AUTO_UPDATE=1` keeps `brew install` and `brew upgrade` from updating the
  taps first: an update would stash the uncommitted formula (and could pull a newer
  tap commit), so the build would test the formula already in the tap, not the new
  render. If the formula was stashed anyway, `git -C "$tap" stash list` shows it; drop
  the stash and copy the render again.
- The formula builds from source on every install (there are no bottles: there is
  nowhere public to host them). The build downloads only the public crate and its
  public dependencies; tapping needs no login.

Users install and upgrade with:

```sh
brew tap cdot65/prisma-airs-cli
brew trust cdot65/prisma-airs-cli
brew install airs-cli
brew upgrade airs-cli
```

## 7. npm

From 8.0.0, `@cdot65/prisma-airs-cli` on npm is a binary-distribution channel in the
layout of `@openai/codex` (DESIGN §21.16): the launcher version `<version>` (the `airs` and
`airs-cli` commands) and one platform version per target, `<version>-darwin-arm64`,
`<version>-linux-x64` and `<version>-linux-arm64`, which the launcher depends on as
aliased optional dependencies. Each platform version holds exactly the released binary:
the signed, notarised macOS build and the static musl Linux builds from the Forgejo
release.

**Stage** after the Forgejo release (the tarballs and `SHA256SUMS` in
`target/dist/<version>/`, or downloaded release assets with `--dist`):

```sh
scripts/npm-package.sh --verify-signature <version>     # writes target/npm/<version>/
cat target/npm/<version>/SHA256SUMS
```

It checks the tarballs against `SHA256SUMS`, stages and packs the four packages, checks
their contents and never publishes. `tests/npm_package.rs` installs packages staged the
same way from a loopback registry and runs `airs --version` and `airs-cli --version`.

**Publish**, only when every check, review and CI job is green. The dist-tags today are
`latest` 7.0.1, `next` 7.3.0 and `forgejo-preview` 7.2.1-forgejo.0. With the npm user
configuration of §1:

1. **The platform versions first**, so the launcher never points at a missing version.
   npm needs a tag for each publish; use `next`, which step 4 moves anyway, so no new
   dist-tag is created:

   ```sh
   cd target/npm/<version>
   npm publish cdot65-prisma-airs-cli-<version>-darwin-arm64.tgz --tag next
   npm publish cdot65-prisma-airs-cli-<version>-linux-x64.tgz --tag next
   npm publish cdot65-prisma-airs-cli-<version>-linux-arm64.tgz --tag next
   ```

2. **Then the launcher**: `npm publish cdot65-prisma-airs-cli-<version>.tgz --tag latest`.
3. **`latest`** is now `<version>`: check with `npm view @cdot65/prisma-airs-cli dist-tags`.
4. **Move `next`** to the same version:
   `npm dist-tag add @cdot65/prisma-airs-cli@<version> next`.
5. **Leave `forgejo-preview` alone.**

Never unpublish: a published version is immutable; fix a bad release with the next patch
version. Verify with `npm install -g @cdot65/prisma-airs-cli` on this Mac and in Linux x64
and arm64 containers (`airs --version` and `airs-cli --version` print `<version>`), and
from an npm 7.x install with `npm i -g @cdot65/prisma-airs-cli@latest` (the `airs-cli`
command must keep working).
