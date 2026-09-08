# Tenant switching and Runtime profile migration — 2026-09-08

## Scope

CLI 5.5.0 adds five `airs tenant` operations and Runtime security profile backup/restore.
SDK 0.29.0 is unchanged: its authenticated Management profile, custom-topic, and DLP
clients already provide the required operations. No browser bearer tokens are used.

## Implementation review

- Tenant registrations reference existing config files, including read-only mounts.
  A separate private, atomically updated registry contains no secrets. TSG identities
  are pinned; malformed/missing selected files and mixed authentication overrides fail
  closed. `default` preserves legacy resolution. Unregister never deletes source files.
- Backup follows complete bounded inventories, selects latest profile revisions, and
  includes exact referenced topic definitions. Versioned JSON/YAML files are private,
  capped at 20 MiB, and never overwrite existing files. No credentials are exported.
- Restore validates a complete dependency plan before writes. Cross-tenant profile/topic
  IDs are newly resolved; unknown identifier fields are refused. DLP mappings are explicit
  and refer to existing target data profiles. Shared topic definitions are never updated.
- Default name conflicts fail. Skip/update are deliberate choices; unattended writes
  require an expected destination TSG. State is checked again before writes; policies
  are read back for exact verification. Partial failures report completed operations
  without unsafe automatic rollback or blind write retries.
- Transfer commands reject debug traffic logging. Schema/parse errors omit payloads.
  The standard `--file-format` flag controls backup encoding; `--output` controls summaries.

## Verification evidence

- Full regression suite: **1,585 tests / 110 files passed**, including the isolated
  two-tenant CLI workflows. Production audit reports no known vulnerabilities; lint,
  formatting, TypeScript, build, and all 14 release-policy tests pass. Seven existing
  non-null assertion warnings remain in the unrelated Runtime command-structure test.
- New tenant module: 100% statements/functions; 96.62% branches. Profile transfer module:
  98.17% statements, 100% functions, 83.19% branches. CLI source is excluded by the existing
  coverage configuration; subprocess integration tests exercise the commands directly.

- 30 tenant unit tests: read-only configs, atomic/private state, corrupt registry handling,
  source preservation, redaction boundary, identity changes, environment isolation, and recovery.
- 31 profile migration unit tests: exact dependencies, pagination budgets, source/destination
  identity, DLP binding/version changes, unknown IDs, stale plans, conflict policies,
  partial errors, and read-back mismatch detection.
- Actual CLI subprocess + SDK OAuth integration against two isolated HTTP tenants: register,
  select, read in all six formats, JSON/YAML backup, no-overwrite, switch, dry-run, restore,
  fresh IDs, source preservation, safe unregister, and pre-auth rejection paths.
- Live built-CLI acceptance: **7/7 checks passed**. All **19 profiles and 26 topic definitions**
  exported; an all-skip dry run covered all 19 profiles. A synthetic topic/profile was
  backed up in both formats and restored under distinct names with verified identities
  and policy. All four synthetic resources were deleted and absence verified afterward.
  The read-only credentials file hash remained unchanged; normal tenant selection was not touched.
- Private live evidence: `artifacts/tenant-profiles-e2e-emIEBd/results.json` and
  `cli-transcript.json`, with raw backups in the same ignored, private directory.
- Docusaurus migration/tenant/reference/release pages updated with actual backup and restore
  output. Strict production docs build passes.

## Honest boundaries

The cross-tenant HTTP/OAuth integration is fully exercised, but live cloud testing has
only one configured tenant. A real two-cloud-tenant migration remains an acceptance check
when a second tenant config is available. DLP resources are not cloned and mapped DLP
definitions require human review for semantic equivalence. Multiple source revisions
sharing a topic name must be split before restoring. The API offers no atomic transaction
or compare-and-swap, so concurrent changes can still race between checks and writes.
Immediate post-write eventual consistency can cause a verification failure; inspect
destination state before retrying. These boundaries are documented in the command guide.

## Assessment

Self-assessment: **9/10 for this bounded CLI feature scope**, not a claim of comprehensive
SDK/OpenAPI coverage or two-live-tenant certification. Regression tests caught and fixed
blank default-format output and an uncaught option-parser error before release. Release
and registry verification are recorded below once complete.
