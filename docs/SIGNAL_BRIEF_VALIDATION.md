# Signal Brief implementation validation

Latest candidate: **8.1.0-rc.3** (profile matrix presentation). The historical RC1 findings below motivated the
remediation recorded in the RC2 section. Stable publication remains on hold for
repeat user-run real-service qualification.

Implementation date: 2026-11-02 (local; final tool logs cross UTC midnight).
Scope is the approved implementation plan. This is an unreleased working-tree
change; no push, release, real tenant collection, new scan or enforcement change
was performed. The pinned SDK checkout and TypeScript history/tags are unchanged.

## Delivered behavior

- Default `airs runtime report` is the standalone Signal Brief, with rolling
  hours/days, repeatable application filters, bounded SDK collection and snippets.
- `runtime scan-logs query` uses session discovery and scan-result enrichment.
  `results --scan-id` and `reports --report-id` expose scanner-only SDK reads.
- `--bundle-dir` publishes a new private directory containing report.html,
  evidence.json and review.md. Files are 0600, directory 0700, no overwriting.
- `--from-evidence` verifies bounded versioned evidence/digest and runs offline.
  `--review-file` imports a collection-bound Markdown review packet, with separate
  agent-assessed and human-reviewed attribution outside immutable evidence.
- Verdict, scan enforcement, detector action, counting units, coverage and source
  windows remain distinct. Intentional monitoring is not treated as a confirmed
  attack or automatic misconfiguration. Missing/failed/unknown outcomes stay unknown.

## Independent review gates

Plan revision independently scored 9/10 before implementation. Every phase 0–6
independently scored 9/10, including final adversarial review. Full review findings
and fixes are retained in [SIGNAL_BRIEF_PHASE_REVIEWS.md](SIGNAL_BRIEF_PHASE_REVIEWS.md).
The final post-fix required workspace run passed **3,674 tests**, with **0 failures**,
5 ignored tests and **no parity skips**. It includes the added attribution, semantic
normalization and formatted-date regression tests.

| Phase | Independent score |
| --- | --- |
| 0 — SDK feasibility and contracts | 9/10 |
| 1 — normalized evidence and privacy | 9/10 |
| 2 — analysis and standalone rendering | 9/10 |
| 3 — bounded SDK collection and scan-log commands | 9/10 |
| 4 — default report integration and private bundles | 9/10 |
| 5 — attributed Markdown review roundtrip | 9/10 |
| 6 — final adversarial implementation review | 9/10 |


## Required checks

| Check | Outcome |
| --- | --- |
| `cargo fmt --all --check` | Passed |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | Passed, including final privacy fixes |
| `cargo +1.89.0 check --workspace --all-targets --locked` | Passed, including final privacy fixes |
| `AIRS_REQUIRE_PARITY=1 cargo test --workspace --locked --no-fail-fast` | Passed: 3,674 passed, 0 failed, 5 ignored, no parity skips |
| `pnpm -C docs-site build` | Passed |
| Generated CLI reference and docs consistency | Passed; included again in final workspace run |
| Signal-specific unit regression suite | 33 passed after post-flight attribution, normalization and timestamp fixes |

Mandatory parity runs use Node 24.22.2, pnpm 10.6.5 and frozen TypeScript 7.3.0 at
`7a54e46eee4f7914d610bc78d2f98dd7de09fbfe`. No parity skips are accepted. Commands use
`mise exec --` for Node/pnpm and `~/.cargo/bin` on PATH. Full logs are retained locally
under `/tmp/airs-signal-phase6-*.log`; they contain synthetic/loopback test output.

## Artifact and visual checks

The actual offline CLI regenerated [examples/signal/](examples/signal/README.md):
plain brief, evidence JSON, review packet, synthetic annotated packet, and reviewed
brief. Collection ID and digest agree across companions; source evidence is unchanged.
All content is synthetic and distinct from the private visual reference.

Local Playwright/Chromium checks ran with network and service workers disabled:
desktop 1440 px, mobile 390 px, keyboard disclosure and A4 print. Zero network requests and zero page
errors. PDF text extraction confirms closed evidence disclosures expand for print,
including the masked snippet, scan verdict/action and attributed assessment. Browser
tooling/previews remain ignored local development files and are not CLI dependencies.

## Qualification limits

SDK identity/report/snippet behavior is proven against pinned 0.2.4 synthetic
fixtures. It is not a claim of real-service qualification. Detail token telemetry
supports only 7/30/60 days; daily collection reports this gap rather than approximating.
Detail sampling is deterministic and coverage-oriented, not a population estimate.
Current profile inventory is tenant-wide and does not establish historical event
policy. Privacy projection is refused when distinct correlation keys would
collapse after masking; ambiguous identities never become authoritative joins. Reviewer metadata is self-attributed; a digest proves integrity, not authenticity.
Masked excerpts can retain confidential prose, so real artifacts remain private.

Any real-service validation remains user-run under the existing isolated live-tier
contract. This implementation did not run the real live gate or retrieve credentials.

## Development binary packages (2026-11-03)

Optimized test builds were prepared after implementation validation, without
publishing a release. Enterprise built Linux amd64 (native GNU and portable static
musl); worf built macOS arm64 over SSH from a checksum-verified isolated source
snapshot. Both requested platform packages passed offline executable smoke tests.
The macOS ad hoc signature verifies; the binary is not notarized. Package checks
verify layout, root ownership metadata, modes, symlinks, byte identity and checksums.
See [SIGNAL_BRIEF_TEST_BUILDS.md](SIGNAL_BRIEF_TEST_BUILDS.md) for paths and commands.
No Forgejo runner was needed for these one-off builds; the existing manual macOS CI
workflow expects a repository-scoped `macos-arm64` runner for future automation.

## Release-candidate version preparation (2026-11-03)

The crate and lockfile now use **8.1.0-rc.1**, targeting stable **8.1.0** after
real-tenant validation. README current-version examples and candidate release notes
match. Historical benchmark and saved-evidence producer versions remain accurate.
Older saved fixtures intentionally identify their original 8.0.0 producer; offline
replay never rewrites that provenance. The README consistency check now compares
against Cargo's validated package version, including prerelease versions.

The previous 8.0.0-labeled development packages are superseded. Release-candidate
rebuilds are complete. Both platform binaries and all compatibility names report
8.1.0-rc.1. Required checks passed again: formatting, Clippy with warnings denied,
Rust 1.89, documentation build, and **3,674 workspace tests** with 0 failures,
5 ignored and no parity skips. Native and extracted-package offline smoke tests
passed on enterprise and worf; macOS signature verification passed after packaging.
The source crate file list includes all new feature modules and excludes local
validation artifacts. No publishing, tags or commits were performed.

## User-run RC1 report review — blockers observed before remediation

The user supplied a saved live-collection bundle for offline review. The agent did
not rerun collection, access tenant credentials or contact tenant APIs. Original
bundle files remained unchanged; private assessment sidecars were generated.

This review exposed two source-level issues that the synthetic checks above did
not catch. **Stable 8.1.0 is on hold pending fixes and repeat user-run validation.**

- Required configuration reads run after session discovery/details under the same
  40% stage deadline. A slow session walk can exhaust that allocation before any
  profile/application request is attempted, while later stage time remains unused.
  Required-source scheduling and realistic latency/fan-out regression coverage need
  correction. A concurrency ceiling does not parallelize sequential await loops.
- Detailed report detector identifiers such as `pi` do not match management
  protection names such as `prompt-injection` under the analyzer's literal comparison.
  Known aliases need grounded normalization while preserving raw provenance and
  unknown/ambiguous states. Current-profile monitoring comparisons are otherwise missed.

The saved bundle also contained unresolved scan-result lookups. Their cause is not
established by generic missing/ambiguous identity notes; do not infer permissions,
retention or a sub-request mismatch without additional evidence. The prior unit,
parity and package-smoke results remain historical validation facts, not proof that
these real-tenant coverage and correlation goals are complete.

## RC2 remediation validation (2026-11-03)

The approved remediation is implemented; independent Codex reviews accepted phases
1–4 at 9/10 each. Source and behavior follow SIGNAL_BRIEF_RC2_REMEDIATION.md. The
original RC1 scheduling and alias blockers have code fixes and executable
regressions. Their effectiveness against the real service still requires the
user's next validation; absent scanner results have no proven root cause offline.

- Required configuration starts before session traversal, with fair independent
  walks, time reservations and actual bounded detail concurrency. A 258-session
  slow fixture completes configuration/inventory and honestly reports its 40-page
  partial detail walk. Ten exact-identity fallback reads are the maximum.
- Rules 2 findings/counts/IDs persist without examples and under HTTP 429. Explicit
  aliases preserve raw flags/report labels and current-control ambiguity. Rules 1
  saved-bundle findings/review bindings and provenance remain compatible offline.
- Examples default to 5 distinct reports, hard maximum 10 and 2 actual serial
  endpoint sends per invocation. Separate SDK operations share the transport guard
  and 429 latch; direct lookup obeys the same endpoint limits. No retries or wait
  for Retry-After. Missing examples alone do not fail strict metadata coverage.
- Exact transaction metadata never supplies scan action/category/report IDs from
  status or extensions. All five correlation fields, zero versus absent subrequest,
  timeout=true suppression and hung-read deadline have loopback regressions.

| Final source check | Result |
| --- | --- |
| Formatting | Passed |
| Workspace/all-target Clippy, warnings denied | Passed |
| Required full workspace/parity suite | **3,692 passed, 0 failed, 5 ignored, 0 parity skips** |
| Rust 1.89 workspace/all-target check | Passed |
| Documentation site build | Passed |
| Generated CLI reference and documentation consistency | Passed in workspace run |
| Slow/rate-limited/missing-result CLI regressions | 26 passed in workspace run |
| Offline browser | 0 network requests, 0 page errors, no mobile viewport overflow |
| Keyboard/print | Both finding disclosures keyboard-operable; printed metadata, raw example labels and assessments present |
| Saved RC1 bundle replay | Offline with absent tenant; strict exit 1 preserved, original files/digests unchanged |

Logs are retained under `.local/dev/signal-brief/rc2-*.log` with machine-readable
check and test summaries. The first broad run caught malformed compatibility-index
citations for two passing parity tests; citations were corrected and the entire
required suite rerun successfully. Final protocol-constant privacy tests cover the
new sampler, transaction source and tool direction without exposing matching payload
text. No new live API collection, credential read or enforcement mutation occurred.

Current synthetic examples are in examples/signal-v2/; examples/signal/ remains the
saved-rules-1 compatibility demonstration. The actual CLI generated the v2 brief,
blank packet and reviewed brief; all example identities and assessments are synthetic.
Desktop/mobile screenshots and A4 PDF were checked locally with network blocked.

The independent final audit initially scored 8/10 after finding a detector-control
normalization counterexample. Publication was held. The fix requires unchanged raw
detector/hint semantics and maps changed direct-lookup labels to unknown; recognized
public spellings survive payload matching without relaxing credential filtering.
New unit, offline prepublication and direct-output tests pass. The full 3,692-test
suite, lint, format, MSRV and site checks above were rerun after this fix.

Final qualifying source manifest SHA-256:
`03ef682932b2115affc35487793f3a8e003e12084d8ef1f38ef3f3cd1241f1cf`.
It covers 257 allowlisted source/build-input files and excludes private bundles.
Both platform builds verified that snapshot. Enterprise rebuilt native GNU and
static musl Linux amd64; worf explicitly recompiled the final source for macOS arm64.
BUILD-INFO records the source manifest and actual per-target executable digests.

Native and extracted-package offline tests pass for both saved rules 1 and current
rules 2 fixtures, including pretenant report limits, private bundles, attributed
review import and no-overwrite behavior. Separate release-binary probes reject
malformed detector controls, hints and example labels before publication. The final
combined macOS archive was extracted again on worf: all three executable names
report 8.1.0-rc.2, strict ad hoc signature verification passes, and both fixture
families plus the alias guard pass. The binary is not notarized. Archive checks
verify exact layout, relative aliases, 0755/0644 modes, root ownership metadata,
README bytes, executable identity and checksums. Packages remain unpublished.

Packages and testing instructions are in SIGNAL_BRIEF_TEST_BUILDS.md. No real
collection was rerun: the final native executable replayed the user's saved RC1
bundle and annotation offline, preserved strict exit 1 and left original files and
digests unchanged. Repeat real-service qualification remains user-run.


The same 26 scan-read integration regressions were additionally compiled with the
executable path bound to each optimized Linux binary: **26/26 native GNU and 26/26
static musl pass** against loopback SDK fixtures, including scheduling, report
budgets/429, exact fallback and strict coverage. These are separate executable
variant checks, not additional unique workspace tests. Logs are
`.local/dev/signal-brief/rc2-release-{gnu,musl}-reads.log`.

Final independent RC2 qualification: **9/10 accepted** by `/root/phase6_review`.
All five remediation phases score 9/10. Source, archive, extracted executable,
version/alias, privacy, required-check and macOS-signature proofs were independently
reviewed; no unresolved material implementation blocker remains. See
SIGNAL_BRIEF_PHASE_REVIEWS.md for the initial 8/10 audit finding, its fix and the
final acceptance. Stable release remains on hold pending repeat user-run live
qualification. No release, commit or registry publication occurred.

## RC3 profile matrix and RC2 saved-run qualification

The profile table is now a matrix with profiles on the vertical axis, detection
services on the horizontal axis and explicit enabled check marks. Raw directional
and action details remain expandable. The matrix source independently scored 9/10;
synthetic regression, paired CLI and browser checks cover state interpretation,
aliases, escaping, bounds, mobile accessibility and wide printing.

The user-run RC2 bundle was reviewed offline. Configuration starvation is resolved
in that run, but the required session-detail source hits the configured aggregate
40-page cap, and scanner results remain unavailable for a subset concentrated in
one application bucket. The report example allowance is not exhausted. Existing
transaction metadata fallback does not establish missing enforcement outcomes.
The pinned SDK qualification example queries both synchronous and asynchronous
scan IDs; an async-only explanation is unsupported. Source inspection also finds
positive detector breakdown fields in session actions that collection currently
ignores; this and sanitized per-application lookup diagnostics are backend follow-up
work. RC3 changes presentation only and does not claim to fix these coverage gaps.

No additional live collection or tenant credential access was performed. Private
saved-run notes and the regenerated matrix view stay with the private validation
bundle; public examples and browser export checks use synthetic fixtures.

RC3 final source checks passed: **3,694 workspace tests, 0 failures, 5 intentional
ignored live tests and 0 parity skips**, plus formatting, Clippy with warnings denied,
Rust 1.89 and the site build. Both optimized Linux variants additionally pass the
same 26 SDK loopback regressions, including matrix delivery. A preexisting fairness
test assumed concurrent requests reached the server in dispatch order; its exact
arrival-order assertion was corrected to require two reads covering both application
buckets, independently reviewed at 9/10. The full suite was rerun successfully after
that test-only correction. Production build inputs were unchanged.

Qualifying build-input manifest:
`47138f005511e06977fdb198644752198bfe71b10fa79d78dfd7c6b4f36eb94d`.
Linux native GNU/static musl and macOS arm64 builds match that 257-input snapshot.
Native/extracted offline workflows test both snapshot versions, profile matrix,
private files, review import, pretenant caps and no overwrites. Malformed semantic
labels are still rejected. The final combined macOS archive was extracted on worf
and verified for strict ad hoc signature, all three executable names, both offline
workflows and semantic guards. No notarization or publication occurred.

Final independent RC3 qualification scored **9/10 accepted**. The presentation and test-build artifacts are complete; unresolved RC2 scan lookup/traversal coverage remains a separate backend qualification item.
