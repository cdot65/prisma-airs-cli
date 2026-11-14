# CLI source preparation and reconciliation

The original Goshawks root selected upstream `fc3a409b60e48431deb620c5525b23a42fffe00e`.
This normal descendant reconciles upstream main through
`71df49f0b085cba880f7cdfc00b26502a38b2240` (40 source commits), preserving the
independent root and all unrelated destination content. Current source is Rust
CLI 9.0.0, Rust MSRV1.89/edition2024; project CI/docs use Node24. This is a source
handoff, not a new release, service deployment or application acceptance.

## Current changes and curated adaptations

Upstream intentionally removed TypeScript tenant/config compatibility and the
reference/parity machinery. Current code, tests, package versions, lockfiles,
notices and documentation are carried together. Later changes add workspace
crate READMEs/MCP documentation and a regression for scanner bindings added after
collection. Third-party dependency versions remain unchanged from the import.

The two changed upstream Forgejo workflows are retained only as inert source
fixtures under `docs/source-preparation/workflows/forgejo/`. `tests/ci_scripts.rs`
has one fixture-path substitution to read the exact retained Linux workflow;
all test bodies and assertions otherwise match upstream. This tests the retained
upstream intent, not an active Goshawks worker. Current concise instructions and
the README preserve the new source home. The current phase ledger retains the
original migration's immutable historical-receipt link correction. Its scores,
outcomes and remaining limitations keep their original scope.

All other incoming application/source/test/lock changes copy exact Git blobs and
modes, including the intentional deletions. Synthetic RSA/withholding fixtures
remain source data; no real tenant, registration, credential store, provider,
service or private recording is read by preparation. The old verbose guidance and
unchanged workflows remain inert historical data, not execution authority.

## Verification and remaining boundaries

Upstream CI for the selected commit reports successful Linux, musl and docs-site
contexts. Those are historical results for the original repository; they are not
newly executed destination CI. The one test-layout adaptation needs a focused
run of the same CI-script tests in an approved isolated environment. Source/data
checks bind all copied/deleted paths, modes, curation decisions and unchanged
synthetic inputs. No project build, application, live endpoint, real registration,
native suite or release command runs on Kirk during preparation.

The new shared workflow, runtime/input admission, protected service/recovery and
publishing authority remain separately gated. `scripts/release.sh` and existing
distribution links still name upstream channels; they must not be activated from
this source handoff. Published8.2/9.0 versions and their bytes are preserved.
No Homebrew, DesktopCLI8.0, MCP deployment, GitHub or registry consumer is upgraded.
See [immutable inputs](IMMUTABLE_INPUTS.md) before retiring old history or inputs.

The [original import record](https://git.cdot.io/goshawks.io/prisma-airs-cli/src/commit/bdc5a7f78706dc005e674dfde50922af595646b1/docs/SOURCE_PREPARATION.md)
remains recoverable at the preserved initial root.
