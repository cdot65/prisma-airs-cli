# Gateway CLI reliability implementation plan

Revision 4. Status: SDK diagnostics, shared CLI preparation, local-failure logging
and the two dry-run commands pass the scoped local synthetic checks. SDK0.2.6
has subsequently been published and the CLI9.1.0 candidate adopts its exact
registry version. CLI diagnostic presentation, service-contract evidence and
wider profile qualification remain pending. See
[current implementation progress](gateway-reliability/IMPLEMENTATION_PROGRESS.md)
and [the historical G0 evidence ledger](gateway-reliability/G0_EVIDENCE.md).
This sequence follows the investigation of the Goshawks production deployment's
`CLI-ISSUES.md` handoff. Phase acceptance follows executed evidence.

The objective is to make gateway administration understandable and predictable:
an operator can construct a valid request, discover why an invalid request was
rejected, preview the effective request, and distinguish a local refusal from a
request that reached the service.

## Scope and current evidence

The work spans `goshawks.io/scm-sdk-rs` and `goshawks.io/prisma-airs-cli`.
The inspected CLI is source version 9.0.0, with `scm-sdk =0.2.4`. Its cached SDK
archive matched the checksum in `Cargo.lock`; the inspected validator, schema
graph, generated models and organisation adapter matched that archive. The local
SDK checkout reports 0.2.5, but the four relevant request/response schema subgraphs
are unchanged. Select a future dependency version from the actual accepted fix.

| Observation | Established explanation | Remaining evidence |
| --- | --- | --- |
| Deployment authentication updates fail locally | The write contract requires a boolean for `allow_all_workspaces` and rejects the read-side `disable_portkey_gateway` field. `grouped_gateway_base_url` is also outside the write contract. | The service's actual update contract, including replacement versus merge behavior. A console-saved record does not identify the console's request. |
| Service-key creation fails locally | `type` is required after input merging; the recorded command omitted it. The timestamp's missing fractional seconds and absent alert emails are not the local blockers. | The service-supported creation vocabulary; a response showing `workspace-service` does not alone establish the create contract. |
| Organisation info fails decoding | JSON parsing, schema validation and typed model decoding all map to a generic response error. | A redacted response shape or equivalent diagnostic from the failing operation. No particular field is yet identified. |
| `--debug` produces an empty file for the rejected writes | Body validation precedes transport, while the logger observes transport attempts. | No additional service evidence is needed to fix this diagnostic gap. |
| Doctor warns about installations | npm launches its bundled executable; Homebrew supplies another executable. Their inspected bytes differ despite the same displayed version. | Exact candidate identity must accompany future reproduction. Different bytes alone do not prove different behavior. |
| zsh rejects a variable holding multiple flags | Scalar expansion does not produce the intended argument vector. | Documentation correction only. |

The original handoff is at
`/Users/cdot/development/talos-cluster/docs/goshawks-prod-deployment/CLI-ISSUES.md`.
It remains historical evidence. New evidence must identify what was observed and
what was inferred rather than silently rewriting that handoff.

Follow [source preparation](SOURCE_PREPARATION.md) and
[immutable inputs](IMMUTABLE_INPUTS.md). Runtime checks require an admitted
environment with synthetic inputs. This plan does not activate Cargo/native,
live/provider, setup/deployment, release or publishing work on the preparation
host. Source reconciliation and this plan do not authorize package publication,
installation changes or production mutations. SDK work also follows that
repository's current input and environment requirements. Subsequent owner
authorization for this session's isolated synthetic execution is recorded in the
current progress/native-check evidence; it does not grant fleet, publishing or
production-mutation authority.

The owner later explicitly requested SDK-first publication and CLI adoption in
this session. SDK0.2.6 has been published with verified registry bytes and an
annotated Goshawks tag; CLI9.1.0 adopts that registry release. This subsequent
release work does not relabel source-preparation, live-service or fleet evidence.

Use current Rust fixtures and tests. The retired CLI TypeScript reference/parity
machinery stays retired; the SDK's independently retained contract inputs follow
the SDK's own rules. Historical CI, live tests and scored phase reviews retain
their original scope and are not acceptance evidence for this work.

## Recommended design

| Module | Responsibility | Interface and constraint |
| --- | --- | --- |
| SDK validation and errors | Interpret the schema once; retain safe diagnostic facts through request validation and response decoding. | Structured diagnostic accessor on SDK errors; existing error kind, remote outcome and resource limits remain authoritative. |
| CLI request preparation | Merge inputs, apply operation semantics, resolve local context and produce the typed request. | One prepared-request interface used by execution and preview. Validation functions take values/context, not `Invocation`, terminals or HTTP clients. |
| CLI execution | Confirm, acquire credentials, reserve secret output, authorize and send the prepared request. | Existing operation-specific execution and one-time-credential handling consume prepared requests. |
| Diagnostic presentation | Explain SDK facts in command terms and emit lifecycle metadata. | Shared safe facts plus gateway-specific flag hints; no second schema validator in the CLI. |
| Preview presentation | Render an explicitly sanitized view of the prepared request and remaining execution requirements. | An operation-specific projection; the preview is never deserialized as an executable request. |
| Test harness | Exercise the production preparation/execution interfaces using synthetic inputs and controlled transport. | Separate schema-conformance tests from independently sourced service-contract fixtures. |

One structural refactor is required before adding preview. Today
`gateway_writes::prepare` mixes body preparation with credential input, destination
checks and terminal gating, while `connect` combines context resolution with
client construction. Extract request preparation and local context resolution
first. Keep CLI I/O and execution in the existing command layer. Preview and
execution are the two concrete consumers that justify this seam; a general
command framework or a new workspace crate is unnecessary.

The initial product scope is field-level diagnostics for gateway schema failures,
correct guidance for the reported deployment/key workflows, the evidence-backed
organisation fix, and offline `--dry-run` on `gateway deployments update` and
`gateway api-keys service create`. Other gateway writes retain execution behavior
and benefit where they already use the shared diagnostic module. Extending
preview to all gateway provisioning, inference or administration commands is a
later capability, not an implicit promise of this change.

Keep advanced JSON input strict. Do not silently coerce arbitrary numbers to
booleans, discard unknown fields, invent key types, or resend a GET response as a
PUT body. The first version retains explicit write-shaped input through existing
flags, `--file` and `--set`. Any service-contract correction belongs in the SDK's
maintained contract source/generator, with provenance and regenerated outputs.

## Phase and dependency map

Each numbered chunk is a reviewable change set with its own evidence. Mark a
phase complete only when its exit criteria are met. Record external evidence or
dependency blockers against the affected chunk; other independent chunks can
continue.

| Phase | Outcome | Chunks | Prerequisites |
| --- | --- | --- | --- |
| G0 | Reproduction baseline and contract evidence | G0.1–G0.3 | Existing investigation |
| G1 | SDK diagnostics retain actionable facts | G1.1–G1.3 | G0.1–G0.2 |
| G2 | CLI preparation separated from effects | G2.1–G2.3 | G0.1–G0.2; can proceed alongside G1 |
| G3 | Gateway errors, logs and command contracts repaired | G3.1–G3.4 | G1 and G2; contract corrections use G0.3 |
| G4 | Offline preview for the two affected writes | G4.1–G4.3 | G2 and G3's diagnostic/contract interfaces |
| G5 | Workflow qualification and implementation handoff | G5.1–G5.3 | Implemented G1–G4; unresolved capabilities remain named |

Critical implementation sequence: G0 baseline → G1/G2 → G3 → G4 → G5.
G0.3 evidence gathering may continue alongside G1/G2. Missing organisation
evidence blocks that repair's acceptance, not the shared diagnostics or refactor.
If it remains missing at handoff, report the organisation capability as unresolved;
do not mark the entire initiative complete.

## G0 — Establish the baseline and authoritative contracts

**G0.1 — Bind source, executable and environment identities.** Record CLI source
revision, exact SDK source/package identity, executable path and SHA-256, install
channel, and the admitted test environment. Select one explicitly addressed
candidate executable for every reproduction; avoid ambient PATH selection. Keep
the published 9.0.0 artifacts and existing installations unchanged. Establish an
evidence ledger linking each issue to its fixture, reproduction command and result.

**G0.2 — Build minimal synthetic reproductions.** Add current-behavior
characterizations at the real CLI/SDK seams. Reproduce the deployment failures
with a wrong boolean and an unknown field independently, then together; cover
the suggested extra `grouped_gateway_base_url`. Reproduce the missing key type
with the otherwise complete command, including its valid timestamp. Assert zero
token/API requests and no created credential file. Pair each refusal with a
write-shaped request that passes the pinned local schema. Keep local validation
success distinct from server acceptance. Capture the empty-debug-log behavior
and define the desired diagnostic assertions before implementing the fixes.

**G0.3 — Resolve the facts that source cannot establish.** Obtain an existing
redacted response shape for organisation info, or schedule an explicitly admitted
operator capture. Preserve required/missing/null/type distinctions in synthetic
fixtures. Establish deployment writable fields and replacement/merge semantics,
plus the service-key create vocabulary, from the applicable SCM contract and
request evidence. Legacy Portkey material and console GET records are leads,
not substitutes for that evidence. Do not perform a production write merely to
capture a request. Any future live check needs its own admitted resources and
scope. Retain provenance and licensing restrictions when deriving fixtures or
generated contracts.

**Exit:** G0.1–G0.2 provide deterministic, agent-runnable reproductions in the
admitted environment and exact expected symptoms. G0.3 records either evidence
or the precise outstanding question for each capability. No guessed response
field or unexecuted test is recorded as confirmed.

## G1 — Add structured diagnostics to the SDK

**G1.1 — Define the diagnostic interface.** Add an optional diagnostic to SDK
errors describing stage (`request_validation`, `response_json`,
`response_validation`, or `response_model`), a canonical schema identifier where
available, and bounded issues. Issues carry schema-owned field paths, a stable
reason code, expected kind/constraint and received JSON kind or absence. They
never carry received scalar values, raw bodies, source expressions, transport
errors or credentials. Represent arbitrary map keys and unrecognized input keys
with a safe placeholder at the nearest known path. Include a static allowed-field
hint when useful. Preserve safe ordinary `Debug`/`Display` behavior.

Use the first decisive issue as the minimum supported result; do not make complete
error aggregation a prerequisite. Any bounded union summaries or additional
issues must be deterministic and explicitly capped. Diagnostic work must remain
within established validation resource limits, and truncation must not replace
the original failure classification. Document compatibility for SDK consumers.

**G1.2 — Populate and propagate diagnostics.** Instrument the existing native
schema walk and typed-model decoding seams. Preserve facts through
`models::support`, the request engine and response contract decoding instead of
replacing every error with a fresh category-only error. Preserve status,
operation/request identifiers under existing sanitization, remote outcome,
normalization, defaults policy and successful wire bytes. Keep malformed JSON,
contract rejection and typed-model failure distinguishable. Do not change schema
acceptance in this chunk.

**G1.3 — Verify diagnostic correctness and containment.** Cover missing fields,
wrong primitive types, null versus omission, unknown fields, arrays, unions,
refinements and typed decoding failures. Include synthetic secrets in values,
unknown key names, map keys and control characters, plus depth/work/size bounds.
Assert that the existing accepted/rejected corpus remains unchanged and that
diagnostics do not turn a post-send decode failure into `NotSent`. Exercise public
typed methods as well as the validator interface. Record an immutable SDK
candidate for downstream integration.

**Exit:** the reported request mistakes have precise, safe diagnostics, response
failure stages remain distinguishable, and the affected SDK compatibility and
resource-bound checks pass. Publishing that candidate remains a separate action.

## G2 — Extract shared CLI request preparation

**G2.1 — Separate input acquisition from request construction.** Keep file reads,
stdin, hidden credential prompts and secret registration in CLI adapters. Feed
acquired values into preparation. Preserve precedence exactly:
`--file` < named flags < `--set`/`--set-string`. Retain limited source attribution
for useful flag/file hints without retaining raw input snippets in diagnostics.

**G2.2 — Separate local context resolution and typed preparation.** Resolve the
selected tenant's noncredential organisation/endpoint context without constructing
an authenticated client. Prepare the typed request using the same refinements
and SDK validation used by execution. Keep early local validation before tenant
loading, then revalidate the final resolved request as needed. Internal placeholders
must never be presented as a fully resolved preview or sent on the wire. Return
the effective method, route, plane, typed body and confirmation/credential-output
requirements through a small prepared-request interface. Bodies remain redacted
under ordinary formatting and are not generally serializable for output.

**G2.3 — Route existing execution through preparation.** Execution acquires the
prepared request, performs the existing confirmation and destination reservation,
authorizes and sends it. Preserve tenant mismatch refusal, cancellation, retry
eligibility, ambiguous-outcome wording and the one-time-credential path that saves
the raw credential response before typed response validation could lose it. Preserve
validation and prompt ordering except for separately documented changes. Remove
superseded paths rather than maintaining two preparation implementations.

**Exit:** affected gateway execution tests pass with unchanged successful request
bodies, request counts, credential handling and confirmation behavior. The pure
preparation seam can be exercised without terminals, credential storage or network.

## G3 — Repair operator diagnostics and the affected contracts

**G3.1 — Adopt the SDK diagnostic interface coherently.** Carry diagnostics through
`airs-runtime-core::error::ErrorFacts`; add one safe formatter usable by local
request validation and gateway response handling. The CLI supplies command/flag
hints. Keep usage errors at exit 2, operational response failures at exit 1,
machine-readable successful stdout clean, and unrelated error presentations
unchanged unless explicitly included in the change. Examples of intended meaning:
`auth_settings.allow_all_workspaces: expected boolean; received number` and
`type: required; supply --type or include it in --file/--set`.

The root CLI, runtime-core, history-store and MCP manifests currently pin the SDK
separately. Adopt one accepted exact SDK version coherently and check all affected
consumers. For prepublication checks use an explicitly isolated integration
checkout with an immutable candidate; temporary overrides stay outside the
deliverable. A release-quality dependency change requires a published immutable
SDK version and its checksum. Preserve unrelated lock entries, package identities
and feature choices. Do not leave permanent path/git overrides or select a
version merely because its number is newer. Coordinate G3.3/G3.4's evidenced SDK
corrections into the candidate before final dependency adoption where their
evidence is available; avoid serial dependency bumps solely to follow chunk order.
If an independent diagnostic release proceeds first, retain the remaining
contract fixes as explicitly pending follow-up changes.

**G3.2 — Log local validation failures as lifecycle events.** Extend the existing
debug module with a small metadata-event interface. A gateway body rejection
after log creation emits a discriminated event with operation, stage, schema,
safe issues and `requestSent: false`. A response-validation event records the
post-send stage without implying that a mutation was rejected. Preserve existing
HTTP-attempt records and distinguish the new event kind so consumers need not
treat it as traffic. Keep argument-parser failures before log initialization as
stderr-only failures and document that scope. Debug logs remain body-free; a
field diagnostic provides the useful evidence without persisting rejected bodies.

**G3.3 — Make deployment and key input contracts usable.** Use G0.3 evidence to
correct the SDK contract only where a mismatch is established, through its
maintained derivation process. Document deployment read/write field differences
and the verified effect of omitted settings. Supply a tested write-shaped example
for changing workspace access while preserving settings the service requires.
Retain rejection of full GET records and numeric booleans unless the actual write
contract proves they are accepted; rejection must explain the known field/type or
the nearest object containing unsupported fields. `--set` constructs a request;
it does not implicitly fetch and patch the stored object.

For service keys, explain required `type` after all input sources are merged and
document verified create vocabulary. Avoid a Clap-level requirement that would
reject valid `--file` or `--set type=...` input. Do not silently default to
`workspace-service` based only on the observed response. Cover timestamp forms
with and without fractional seconds and optional alert-email omission. Update
help and examples together with these tests.

**G3.4 — Fix the organisation response using its specimen.** Minimize G0.3's
redacted shape to the actual failing constraint, then add a regression through
the typed organisation client and CLI read. Change only the justified contract
or decoding behavior. Preserve unknown response data and omission/null distinctions;
do not fabricate settings or replace the typed read with an unrestricted raw
JSON bypass. If the failure is malformed JSON, retain failure and verify the
new accurate diagnostic instead of weakening the schema. If no specimen is
available, leave this chunk pending and name the missing evidence.

**Exit:** the original local mistakes explain what must change, their debug files
contain useful metadata, and the documented write shapes agree with the admitted
contract evidence. Organisation acceptance requires its actual failure class to
be demonstrated and addressed; synthetic guesses do not close it.

## G4 — Add offline preview for the reported writes

**G4.1 — Specify and expose `--dry-run`.** Add the flag only to deployment update
and service-key create. Preview performs local input/context resolution and the
same typed preparation as execution. It performs no token acquisition, catalog
or resource lookup, HTTP call, confirmation prompt or secret-output reservation.
Local input files and selected tenant metadata may be read; tests use synthetic
stores containing the required noncredential context. A tenant's missing management
credentials must not prevent a locally complete preview. If required local
context cannot be resolved, report the problem without silently accessing the
network or presenting placeholders as resolved values.

Reject `--dry-run --force` on deployment update. A service-key preview may omit
`--secret-output` and must state that a destination is required to execute. If a
destination is supplied, perform normal read-only destination checks but never
reserve or create it. Local validation failure retains exit 2; configuration or
file failures retain their existing categories. A successful preview exits 0
and explicitly says it validates local preparation, not service authorization or
acceptance. Optional debug output remains the only diagnostic file write.

**G4.2 — Render a safe prepared-request view.** Support existing human output
conventions and a versioned JSON preview with effective method/route/plane,
resolved tenant context, sanitized body and execution requirements. Use an
operation-specific request projection built on existing withholding primitives;
response policies are not automatically correct for request bodies. Validate
the real input first and redact only the output projection. Withhold credentials
and arbitrary unsafe content, including credential-bearing URL parts and dynamic
keys. Mark redaction clearly. Neither the printable view nor its JSON form is
an apply token or a reusable request file.

**G4.3 — Prove preview and execution agree.** Compare the prepared body and the
recorded request from fixture-backed execution for each supported command and
input source. Test invalid bodies, tenant mismatch, local context failure,
redaction, quiet/output modes and confirmation combinations. Prove zero network
and zero secret-output creation in preview using an observable trap transport
and temporary filesystem assertions. Prove preview never prompts or blocks on
implicit credential input. Follow a successful preview with separately invoked
execution in the fixture and verify the expected write count and credential save.

**Exit:** both operator workflows have useful offline previews based on the same
prepared requests execution sends, with explicit and tested limits on preview's
side effects and guarantees.

## G5 — Qualify complete workflows and hand off

**G5.1 — Exercise the incident workflows through the real CLI.** Run the targeted
matrix below in the admitted synthetic environment. Improve the existing harness
only where the relevant workflow needs state: deployment update/read-back should
reflect the recorded contract rather than always returning a fixed success.
Assert key creation's admitted request contract and one-time response handling.
Keep service-derived specimens distinguishable from fixtures invented to satisfy
the SDK, so a mock's success is not mistaken for real service acceptance.

**G5.2 — Review shared impact and update maintained contracts.** Run focused SDK
validator/engine tests and CLI gateway writes, inventory, debug, error-mapping and
withholding tests. Because SDK errors and runtime-core are shared, also check the
affected MCP/history consumers and representative non-gateway error paths.
Select formatting, lint, MSRV, feature and broader compatibility checks from the
actual diff and each repository's admitted profile. Preserve prior evidence with
its original binding; broaden testing when a change or failure justifies it.
Update DESIGN D38/D116/D233, the debug-record contract, compatibility notes and
gateway help/reference docs. Retain historical decisions and explain their new
replacement instead of relabeling old results.

**G5.3 — Produce the acceptance and release-readiness handoff.** Record source and
SDK revisions, candidate executable hashes, exact commands/results, fixture
provenance and unresolved capabilities per chunk. Update operator guidance to
identify the executable actually running and to use zsh argument arrays or
literal flags. Explain doctor's multiple-installation warning; do not silently
uninstall or switch channels. Separate synthetic implementation acceptance from
real-service qualification and published-distribution acceptance. Any later
service check uses a specifically admitted disposable resource and execution
scope; publication and install-channel promotion follow their own authorization.

**Exit:** the implementation evidence is reviewable, every claimed repair has a
regression at the appropriate seam, and the handoff clearly identifies which
capabilities are implemented, service-qualified and published. A pending
organisation diagnosis, unavailable SDK release or unperformed service check
is reported as such, never converted to a pass.

## Required regression matrix

| Scenario | Required observation |
| --- | --- |
| Recorded deployment body | Local refusal identifies the wrong boolean or unsupported-field location; zero OAuth/API requests. Independently fixing each blocker exposes the other. |
| Added `grouped_gateway_base_url: null` | The pinned-contract refusal is explained; later acceptance requires an evidenced contract change. |
| Valid write-shaped deployment settings | Typed preparation succeeds; fixture execution preserves the confirmed update semantics, sends one write on ordinary success and retains the existing narrowly permitted refresh/replay behavior. |
| Full deployment GET document supplied as input | Clear refusal or an explicitly evidenced supported mapping; no silent dropping of fields. The proposed first version refuses. |
| Missing service-key type | Actionable post-merge error; no token request, key creation or output-file reservation. |
| Type supplied by flag, file or assignment | Equivalent prepared body when inputs are equivalent; documented precedence when they differ. |
| Valid expiry with/without fractions; omitted alert emails | Both timestamp forms and omitted optional emails pass local validation. |
| Organisation invalid JSON, schema failure and model decode failure | Distinct safe stages; no echoed body or secret; the observed incident specimen selects the actual regression. |
| Preflight rejection with `--debug` | A metadata validation event with `requestSent: false`; no fabricated HTTP attempt and no body. |
| Post-send decode failure | Diagnostic added while remote outcome, request count and one-time-credential preservation remain correct. |
| Dry-run on either supported command | Same preparation as execution; zero transport/auth/prompt/secret-output side effects; explicit preview result. |
| Synthetic secrets in values, keys and URLs | No secret disclosure in stderr, preview, debug records or ordinary formatting. |
| SDK dependency adoption | One intended exact SDK version across consumers; unchanged unrelated dependency resolution and existing feature constraints. |
| Multiple install channels | Test/evidence names the exact binary; a shared version string is never used as hash-equivalence proof. |

## Source map for implementers

CLI paths are relative to this repository. SDK paths are relative to
`goshawks.io/scm-sdk-rs`; consult the exact candidate revision before editing.

| Area | Primary source/test locations |
| --- | --- |
| CLI input and body validation | `src/cli.rs`, `src/commands/gateway_write_plan.rs`, `src/gateway/body.rs`, `src/gateway/writes.rs` |
| CLI preparation/execution | `src/commands/gateway_writes.rs`, `src/gateway/secrets.rs`, `src/commands/mod.rs`, `crates/airs-runtime-core/src/clients.rs` |
| Organisation read | `src/commands/gateway_inventory.rs`, SDK `crates/scm-sdk/src/airs/gateway/organisations.rs` |
| Shared errors and debug | `crates/airs-runtime-core/src/error.rs`, `crates/airs-runtime-core/src/debug.rs`, `src/debug.rs` |
| CLI regressions and harness | `tests/gateway_writes.rs`, `tests/gateway_inventory.rs`, `tests/debug.rs`, `crates/airs-test-harness/src/fixtures_gateway.rs`, `crates/airs-test-harness/src/server/gateway_writes.rs` |
| SDK diagnostics | `crates/scm-sdk/src/error.rs`, `crates/scm-sdk/src/validation.rs`, `crates/scm-sdk/src/models/support.rs`, `crates/scm-sdk/src/engine.rs` |
| SDK contract ownership | `contracts/`, maintained generation in `tools/`, generated models/schema graph under `crates/scm-sdk/src/`; preserve frozen sources and notices |
| SDK verification | `crates/scm-sdk/src/validation/tests.rs`, engine/error unit tests, `crates/scm-sdk/tests/gateway_contracts.rs`, `gateway_control_contracts.rs`, `schema_contracts.rs` |
| Installation identity | `src/commands/doctor.rs`, `npm/lib/launcher.js`, their existing tests |
| Current user documentation | `docs-site/docs/commands/gateway.md`, `docs-site/docs/reference/commands/gateway.md`, `docs/DESIGN.md`, compatibility records |

At each chunk closeout, append its source revision, affected checks and outcomes
to the evidence ledger created in G0.1. Update this document's overall status as
work progresses. Prepared source or documentation alone does not accept a phase.
