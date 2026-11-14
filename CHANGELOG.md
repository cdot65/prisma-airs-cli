# Changelog

Release notes for each version are in [docs/RELEASE-NOTES/](docs/RELEASE-NOTES/). This
file records the changes to the repository itself.

## Unreleased

- **Gateway reliability, 9.1.0 candidate.** Shared typed request preparation,
  deployment-update and service-key-create dry runs, privacy-protected local
  validation lifecycle logging, and updated Docusaurus guidance/reference.
  Release notes: [9.1.0](docs/RELEASE-NOTES/9.1.0.md).

- **Performance documentation.** docs/BENCHMARKS.md is re-measured for 8.0.0 against the
  TypeScript CLI 7.3.0 (2026-10-23), with the executables' sizes per platform, their growth
  from 0.1.0 and what that growth costs. README gains "Why Rust: performance", the docs site
  a Performance page, and the 8.0.0 release notes a Performance section.

## 8.0.0 (2026-10-22)

### The Rust CLI replaces the TypeScript CLI in this repository

- **Source.** The Rust CLI (crate `airs-cli`, command `airs`, version 1.0.0) was copied
  from `cdot/prisma-airs-cli-rs`
  (`ssh://git@git-ssh.cdot.io/cdot/prisma-airs-cli-rs.git`) at
  `a9d7864f037cc3bf0d2ef6de2c8ac6341ce3842a` with `git archive`, without its history.
  Its release notes up to 1.0.0 are in `docs/RELEASE-NOTES/`.
- **Removed from the tree.** The TypeScript CLI (`@cdot65/prisma-airs-cli` through
  7.2.1-forgejo.0 on `main` at `5711012`): its sources, tests, build and release tooling,
  configs, lockfiles, TypeScript-only workflows (`docker-publish.yml.disabled` included)
  and assessment notes. They stay in the git history and in every `v7.*` tag (7.3.0 is
  `v7.3.0`, `7a54e46`); the parity tests build the TypeScript reference from that history.
- **Added.** The `airs-cli` name for the Rust CLI (D330), the doctor's `Executable` check
  (D331), the npm launcher package and `scripts/npm-package.sh`, the Rust docs site in
  `docs-site/` with a command reference generated from `--help`, and `NOTICE` in the
  crate, the release tarballs and the npm packages.
- **Release.** One 8.0.0 release on crates.io, npm, Homebrew, the Forgejo release and the
  docs site, after the live run of the 7.3.0 port and its review. There is no 1.0.1 or
  1.1.0 release. Notes: `docs/RELEASE-NOTES/8.0.0.md`.

### The TypeScript CLI 7.3.0 port

- **Reference.** The parity reference moves to tag `v7.3.0` (`7a54e46eee4f`, TS SDK
  0.35.0), the final TypeScript release (docs/TESTING.md §3.4).
- **SDK.** `scm-sdk = "=0.2.4"` from crates.io, the TypeScript SDK 0.35.0 overlay (DESIGN
  §15.25). The port was first built on its exact Forgejo commit `4825616` as a temporary
  git dependency; the published crate's sources are that commit's, with the version
  bumped. Requests carry `PAN-AIRS/0.2.4-rust-sdk`. `Cargo.lock` also moves
  `rustls-webpki` to 0.103.16 and `webpki-root-certs` to 1.0.10 (RUSTSEC-2026-0382,
  -0383); `cargo audit` and `cargo deny check advisories sources` pass.
- **The 0.2.4 fields.** A profile answer whose new fields hold an unexpected value (an
  explicit `null`, a wrong type) is printed and backed up as sent (D334); every new
  request field is checked before the tenant; omitted settings stay omitted and an explicit
  `false` stays `false`.
- **Response defaults on every profile read** (no output change, DESIGN §15.25): `profiles
  list|get`, the `--all` and lookup walks, `profiles update|delete` lookups, the `topics
  apply|revert` profile reads, `profiles cleanup` and the `profiles create|update` and topic
  PUT answers are validated without the SDK's response defaults, like the backup, restore
  and report already were; a restore re-checks each mapped body with the 0.35.0 request
  root. New regressions pin multi-page lossless reads and backups, the update's debug
  operation names, `targets error-logs --skip` as a usage error, the 0.2.3 profiling
  statuses, closed target unions, the `api.apps` endpoint defaults and `serde_json`
  without `preserve_order`.
- **Directional security profiles** (DESIGN §13.17): `--direction` and
  `--ai-profile-index` on `runtime profiles create|update` and `runtime topics
  apply|revert`; `--[no-]enable-full-conversation-inspection`, `--no-mask-data-inline`,
  `--no-mask-data-in-storage`; `create --config` names the profile from the file;
  `--config` is exclusive with every write flag; the recursive merge; the layout refusals
  (exit 2); the profile views, the runtime report and profile backup and restore read every
  direction. Accepted by a live run against a tenant (2026-10-22) for each of the four
  directions. A flag create with `--direction` writes the other three directions as empty
  objects, because the service refuses a directional profile without all four (D335;
  TypeScript 7.3.0's one-direction body is refused with HTTP 400). A directional flag create
  whose flags fill no direction (none, or shared settings only) is refused before OAuth
  (exit 2) with a text naming the service rule (D336; TypeScript 7.3.0 sends a shared-only
  create, which the service refuses, and its text for a flagless one points at `--config`,
  whose empty policy the service refuses too).
- **Known limitations of directional profiles.**
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
- **The rest of 7.3.0.** Highest-revision profile lookups in `topics apply|revert`; ref and
  group metadata kept; every profile body validated with the 0.35.0 request schema;
  `profiles create|update` print a status line and the banner-less detail and honour the
  tenant's `defaultOutput`; `--active` only when given; the `malicious-code` name on the
  update merge path; `--dlp-profiles` and `--[no-]mask-data-inline` without `--dlp-action`
  on update; `revert` refuses a topic the profile does not reference.
- **§12 rows.** New: D332–D336. Retired: D117. Reworded: D26, D50, D93, D94, D96,
  D98–D103, D106, D115, D116.
