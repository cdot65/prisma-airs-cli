# CLI9.1.0 integration with published SDK0.2.6

The owner requested SDK-first publication, then CLI registry adoption and a
versioned source merge. SDK0.2.6 was published first; its downloaded bytes match
SHA256 `382715ffd429bf4c4b02364131c6bd183775d29e127cc2f3049c56043ee203e6`.
The CLI pins that immutable registry artifact in all four SDK manifests.
Five versioned workspace crates move together to 9.1.0. No other third-party
versions, pnpm locks, frozen inputs or distribution-channel identities change.
CLI distribution artifacts and MCP deployments are not published by this merge.

## Executed local checks

These checks ran from an isolated source snapshot with cleared environments,
synthetic/loopback fixtures and locked dependencies on macOS ARM64.
[Source, compiler, command, binary and log bindings](release-9.1.0/qualification.json)
retain each result; `retained_log` resolves beside that receipt. The final source
manifest matches the current implementation. One stale SDK-version comment was
corrected after the first integration runs; the affected gateway_inventory target
was replayed on both compilers, with 43 passing each. No runtime behavior changed
in that follow-up.

| Selection | Rust1.89 | Rust1.99 |
| --- | --- | --- |
| CLI library | 694 passed; one helper ignored | 694 passed; one helper ignored |
| Runtime-core library | 240 passed | 240 passed |
| Gateway writes/reads/inventory/report/inference, management, runtime writes, debug, generated command reference | 406 passed | 406 passed |
| Version-sensitive packaging/npm/Homebrew/docs/program-name/CI-script checks | 40 passed | 40 passed |
| History-store library | 107 passed; four opt-in scale/memory checks ignored | 107 passed; four opt-in scale/memory checks ignored |
| MCP-client library | 9 passed | 9 passed |
| MCP-service library | 114 passed; 56 failed at storage admission | 114 passed; 56 failed at storage admission |

The root-library ignored helper is a subprocess entrypoint used by parent lock
tests. The four ignored history-store cases are opt-in million-scan report/restore
and large-transfer memory qualifications; these were not executed. Workspace/all-target Clippy passes with warnings denied
on Rust1.99. The retained candidate reports version9.1.0. Docusaurus typecheck and
production build pass on Node24.21.0/pnpm10.6.5 with the unchanged frozen lockfile;
raw typecheck/build logs are now retained with hashes in the receipt.

## MCP platform refusal remains visible

The full workspace test matrix is **not green**. All 56 MCP-service failures on
each compiler are `UnknownFilesystem` before SDK/client construction. A minimal
one-test reproduction fails the same way. The storage classifier accepts Linux
filesystem magic values; its source is byte-identical to the base revision
`e235a4d0aa431eb0929c129f0ae0f6d008b07d7c`.
[The failure inventory and source hash](release-9.1.0/mcp-macos-platform-refusal.json)
bind this diagnosis. Production storage admission was not changed or bypassed.
The receipt deliberately sets aggregate `passed:false` and
`all_workspace_tests_passed:false`, separately from `cli_affected_checks_passed:true`.
Linux storage-dependent service acceptance remains unqualified; no MCP deployment
is included in this CLI source merge.

Other platforms/fleet, the retained older-binary M7 integration, live contracts,
CLI distribution publication and richer diagnostic presentation through the CLI
error bridge remain outside these executed checks. Existing historical receipts
retain their original source and scope.

## Version-bump impact selection

The six additional root targets cover release-script parsing and stub tarball
layout, local npm staging/launcher/install behavior, fixture-based Homebrew
rendering, README/design/compatibility consistency, program aliases/version/help,
and the retained CI-script/status fixtures. They pass on both compilers. These
are synthetic packaging checks: no real channel is published, and the retained
workflow fixture is not a new runner qualification. An initial README consistency
failure caught candidate wording inside the parsed version field. Moving that
wording to a separate sentence fixed it; the original failure log is retained as
`release-9.1.0/cli-version-surfaces-initial-readme-failure.log`.

Together with the nine gateway/management/debug/reference targets and full CLI/core
unit libraries, this selection covers the changed runtime preparation, previews,
diagnostics, SDK pin/user agent, coupled versions and source documentation. The SDK
package itself passed its all-feature/minimal/doctest/consumer selections on both
compilers before publication. Other root integration targets cover unchanged
command families or separately admitted old-binary/service/scale capabilities;
this record does not claim a complete root or workspace test pass. Workspace
Clippy is explicitly scoped to Rust1.99. The owner has now accepted the CLI-only
source merge with Linux MCP qualification deferred, after the explicit merge
choice and filesystem explanation. [The recorded decision](release-9.1.0/owner-merge-decision.json)
resolves that source-merge prerequisite. The MCP failures remain failures, and
Linux service acceptance and deployment remain separate requirements.

The [final actual Claude Code review](RELEASE_REVIEW_FINAL.md) scored9/10 with
no required source changes for the owner-accepted CLI-only merge. The reviewer
verified source/receipt hashes independently, without independently executing the
tests. The failed MCP results and remaining capability limits are unchanged.
