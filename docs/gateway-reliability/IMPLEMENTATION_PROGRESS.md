# Gateway reliability implementation progress

Entry native-2 supersedes the earlier source-only status. After the owner supplied
the test environment in this session, the scoped changes were compiled and tested
from isolated source copies with synthetic/loopback fixtures on Rust1.89.0 and1.99.0 macOS
ARM64. This is local execution evidence, not fleet, live-service or release
acceptance. Earlier review reports and their source hashes remain historical.

[CLI9.1.0 registry integration](INTEGRATION_9_1_0.md) records the subsequent
published SDK adoption, completed local CLI checks, and the unchanged MCP Linux
filesystem admission refusal on macOS. The full workspace matrix is not green.

## Current implementation and verification

| Plan work | Current implementation | Executed evidence / remaining work |
| --- | --- | --- |
| G0.2 | Incident reproductions and independent local-contract cases. | Included in the 80 passing gateway-write tests. Archived baseline and actual G0.3 service receipts remain separate. |
| G1.1–G1.3 | Value-free SDK diagnostics, bounded root-prefix paths, probe suppression, multipart paths and response-stage/metadata preservation. | Initial review: 217 SDK library tests in the CLI feature profile, including 17 diagnostic regressions plus an external API consumer. Published0.2.6 package qualification: 231 all-feature tests, 150 minimal-feature tests, 13 doctests and one external consumer on each Rust1.89/1.99; all-feature Clippy passes with warnings denied. Other platforms remain unexecuted. |
| G2.1–G2.3 | Pure prepared requests own tenant binding and typed parsing; execution/preview share preparation. | 80 gateway-write integration tests and 87 gateway-related CLI unit tests pass, covering existing writes, confirmations, provider resolution and credential preservation. |
| G3.2, partial | One adapter logs typed-validation and tenant-binding refusals with static operation, stage, checkpoint and requestSent:false. | Integration coverage plus 15 runtime-core debug tests pass. SDK field-fact adoption through the CLI error bridge remains G3.1 work. |
| G3.3, local guidance | Post-merge key-type help, read/write shape guidance and local timestamp forms. | Generated reference verified. Actual service-contract correction still requires G0.3 evidence. |
| G4.1–G4.3 | Deployment-update and service-key-create dry-run use the typed serializer and sanitized projection without clients, authentication, prompts or destination reservation. | All 25 incident/reliability tests pass within the full write suite; seven preparation and three projection unit tests pass in the gateway selection. |
| G5.2, documentation | User guide, design/compatibility notes, SDK diagnostics docs and generated gateway reference updated. | Reference generation and check each pass two tests. CLI Clippy passes with warnings denied. Source formatting, whitespace and local link checks pass. The subsequent Node24/pnpm10.6.5 Docusaurus typecheck and production build also pass; fleet and distribution-channel qualification remain separate. |

The initial reviewed snapshot used the exact scm-sdk0.2.4 dependency. The owner
subsequently requested SDK-first publication and CLI adoption. SDK0.2.6 is now
published and verified, and the CLI9.1.0 candidate pins that registry release in
all four manifests. The workspace versions, SDK version stamp, user-agent
assertions and Cargo.lock follow those deliberate updates; other third-party
versions and the pnpm lock are unchanged. The CLI error bridge has not adopted
the diagnostic getter; current CLI stderr still uses existing schema messages.
The generated CLI reference was regenerated from the tested candidate, rather
than edited by hand. Release integration checks are recorded separately from the
earlier source-bound native evidence below.

[CLI native checks and exact hashes](NATIVE_CHECKS.md) record commands, raw logs,
source bindings and binary identity. The sibling SDK has the corresponding
`docs/GATEWAY_DIAGNOSTICS_NATIVE_CHECKS.md`. Initial native runs caught a CLI error
conversion and a synthetic deep-fixture stack assumption missed by source review;
both are fixed, and the final complete selections pass. Feature-off SDK helpers
are gated to keep its minimal build warning-free.

The owner accepted the CLI-only source merge with MCP Linux qualification
deferred; [the recorded decision](release-9.1.0/owner-merge-decision.json) preserves
the failed MCP results and grants no service deployment or CLI distribution
publication. The [final Claude release-integration review](RELEASE_REVIEW_FINAL.md)
scored9/10 with no required source changes for this accepted merge scope.

## Remaining integration and acceptance

1. Registry dependency adoption is complete in the candidate. Present structured
   diagnostics through the shared CLI error bridge, then verify field hints and
   richer lifecycle events. This presentation work remains open; no cached-crate
   modification or permanent path override is used.
2. Close the actual organisation-response mismatch, key-type service vocabulary,
   and deployment replacement/merge semantics using G0.3 service evidence. Local
   schema and fixture results do not establish those contracts.
3. Exercise other admitted platform/profile checks after the passing MSRV1.89 and reviewed1.99 local runs, record
   the applicable phase decisions, and perform any separately authorized release
   work. These local checks do not publish, promote or qualify a fleet.

## Historical source-only iterations

The following entries describe their earlier snapshots, before the native runs
above. Their statements that tests were unexecuted apply to those snapshots.

## Claude Code review iteration 2 remediation

The first source-only reviews scored CLI 6.5/10 and SDK 7/10; their original
reports remain preserved. Changes for the next review:

- SDK speculative union/ordered probes suppress diagnostics, with one fixed final
  metadata allowance independent of acceptance budgets. Paths keep the root prefix
  when truncated. Added actual interpreter truncation, discarded-probe, anonymous
  array, unknown-response, depth/size and response metadata regressions.
- CLI pure preparation classifies schema and tenant refusals. One command adapter
  logs preflight, binding and provider resolution failures consistently. Client
  construction follows tenant/body binding; credentials remain execution-only.
- Preview applies shared resource paths before nesting, tracks policy redaction,
  and defensively guards printable identifier/time/boolean shapes. Added specific
  refusal assertions, dry-run debug, flag/body tenant mismatch, rotation, pretty
  output, quiet JSON, URL/label withholding and root-policy regressions.

All added tests are source, not executed evidence. Cargo/native/live checks remain
pending the admitted environment described above. A fresh Claude review follows;
its actual score is recorded separately without changing the historical reports.

## Claude Code review iteration 3 remediation

Iteration 2 scored both repositories 7.5/10, while explicitly reporting some
unread evidence. Its reports and exact snapshot hashes are preserved separately.
The next review must read the full regression selection and focused frozen-graph
and generated-parser excerpts before assessing missing coverage or symbol risks.

The source now narrows lifecycle documentation to typed-schema and tenant-binding
refusals, documents the known-literal preview policy, and withholds a malformed
scope shape defensively. A provider-catalog fixture regression exercises the final
preparation checkpoint after reads but before mutation. The SDK committing-walk
invariant is explicit and a recovered probe is covered. The engine now copies
only value-free diagnostics into a fresh remapped error, then attaches the actual
response metadata; a focused test prevents accidental validator-detail promotion.
Native tests still have not run; source-review scores do not close that gate.

## Claude Code review iteration 4 remediation

The full iteration-3 reviews scored CLI 8/10 and SDK 8.5/10, finding four low
items and no critical/high/medium defect. Both reports and snapshot hashes are
preserved. Selected tenant format validation now belongs to pure tenant binding
and is identical for preview and execution, with unit/integration regressions.
The runtime-core debug primitive has direct tests for all stage/checkpoint names,
exact record shape, case-insensitive secret suppression and continued logging.
Multipart metadata validation now prepends the trusted schema field name, with
a synthetic binary-channel regression. The hosted-flag union comment now names
the actual frozen numeric literals. These new regressions remain unexecuted.

## Claude Code review iteration 5 remediation

Iteration 4 scored CLI 8/10 and SDK 8.7/10. Source fixes now describe exactly which
identifiers/timestamps/slugs preview displays, and warn that workspace slugs may
identify projects/customers. JSON object and multipart unknown-field diagnostics
share one budgeted constructor; multipart catchall errors use `MapMember`, and a
synthetic stray-key regression checks that no caller key/value is retained.
The CLI reviewer explicitly held its score below 9 partly for unexecuted native
checks and intentionally deferred SDK/service integration. An admitted synthetic
test environment or an explicit owner override of the preparation-only execution
ban has been requested. No pending check is presented as a pass.

## Native iteration 2: qualification follow-up

The SDK now tests a129-byte trusted field through the interpreter and a real
GatewayDeploymentUpdateRequestSchema at default depth64 on the default worker.
Its full216-test library suite passes on Rust1.89 and1.99. A fine-depth comparison
found that the first wrapper version reduced synthetic
stack headroom. Removing the recursive wrapper fixed it: the final candidate
passes depth55, versus54 on the unchanged base. Both still overflow the artificial
trusted graph at56/64; the evidence preserves this limitation without claiming
arbitrary-schema stack safety. Diagnostics attach at existing core/effect exits.

CLI79 integration,86 gateway unit,14 debug unit and2 reference tests pass on both
compilers. Clippy is clean on the reviewed1.99 profile. A candidate local dry-run
using the owner-provided vulture context also succeeds without sending a request.
The MSRV toolchain was isolated, but rustup auto-updated its shared launcher from
1.29.1 to1.29.3; this incidental tooling change is recorded in native evidence.

## Iteration 8 follow-up

Focused interpreter cases now cover record-key/value failures, pipelines,
intersections, committing discriminated unions and both effect boundaries,
asserting one final diagnostic allowance. Frozen-ID probes assert graph shapes;
multipart cases use the stable public schema name. A standalone external native
consumer names and uses all five diagnostic types through scm_sdk::error.
Documentation deliberately scopes stage coverage and the canonical namespace.

The pure preparation layer gives an explicit missing-mgmtTsgId configuration
error. The native integration case also confirms that a registered tenant whose
TSG disappears is rejected earlier by the existing identity pin in both execution
and preview; that protection is unchanged. Privacy suppression of lifecycle
records remains intentional and is now described in the guide.

## Iteration 9 follow-up

Lifecycle logs now fall back to a body-free redactionFallback event, or a generic
marker, when a registered secret matches static details. All variants retain the
same whole-line privacy guard; a collision with every safe marker is still
suppressed. The direct debug selection now has15 passing tests on both compilers.
Displayable preview literals are explicitly a privacy policy, and zero dry-run
exit status certifies local preparation rather than readiness to execute.
The SDK guide explicitly documents opaque graph-local schema IDs and best-effort
reason/expected-kind hints for composite constraints.
