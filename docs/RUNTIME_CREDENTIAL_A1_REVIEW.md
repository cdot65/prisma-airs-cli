# A1 application-bound scanner implementation review

Reviewer: independent Codex instance `/root/phase5_review`, 2026-11-04.
Scope: `RUNTIME_CREDENTIAL_CONTRACT.md`, DESIGN §3.15 and the approved A1 gate in
`RUNTIME_HISTORY_IMPLEMENTATION_PLAN.md`. Review is based on implementation and
loopback/synthetic evidence. No real tenant, credential, private saved customer
bundle, live service, SDK checkout, commit or publication was accessed or changed.
Only this review document was written by the reviewer.

Initial score: **8/10 — revise before acceptance**.

## Blocking findings

1. New local scalar options and scanner selectors silently retain last-value-wins
   behavior from the root CLI's `args_override_self=true`. The dedicated scanner
   commands do not override that setting, and `evidence_repetitions` originally
   excludes the tenant scanner group and the `--scanner` option. Independent
   isolated probes established that repeated `--file` reads the final file and
   exits operationally rather than rejecting usage; repeated `--app-id` reaches
   tenant lookup; and repeated direct `--scanner` reaches tenant selection.
   This violates the explicit scalar-repetition contract and can silently change
   the selected input, application association or credential. Reject repetitions
   before input/store access and pin local leaves plus direct scan/bulk/lookup
   selectors without rejecting intentionally repeatable IDs. The root has added
   a failing regression while addressing this finding.

2. The A1 acceptance gate explicitly calls for old-native-binary refusal of a new
   named table. The frozen TypeScript parity test proves unknown command/option
   behavior, but cannot prove the pre-A1 native TOML schema behavior. Record an
   isolated frozen pre-change native compatibility probe: old scalar config can
   be read, named config is refused without writes, and explicit exact-byte
   rollback restores old-reader access. No real store is needed. The A3 phase
   still owns its broader extracted-package/state-version qualification.

## Implementation assessment

The strict named namespace preserves scalar-only canonical output, exact names
and application IDs, canonical admitted endpoints, retry bounds, independent
revision references and exactly one credential kind. Case-ambiguous names,
duplicate app bindings, unknown nested fields and malformed values fail with
fixed namespace errors. File/stdin input is bounded and consumed before store
access or locks. Public metadata excludes secret fields, registers every named
secret (including unused sets), scrubs coincidences and refuses projected key
collisions. Generic scalar mutations preserve the namespace and cannot mutate it.
Named-only doctor behavior explicitly skips guessing a scanner probe.

The first format upgrade snapshots exact previous bytes through Transaction,
with exclusive private staging, file and directory fsync, no-replace publication
and ownership-aware rollback. Restore checks selected store/prefix, private file
and registered TSG, rereads under the lock and retains a pre-restore copy. No
ordinary reader locks, writes or repairs the store. Empty namespaces remain explicit
following removal. Existing-store identity checks and directory safety primitives
are reused rather than introducing a second configuration source.

Management discovery groups by the explicitly associated dashboard application ID;
it does not infer association from names or try flat/alternate credentials.
Per-app clients carry only their selected scanner secret and admitted endpoint.
The full event identity retains exact app-name/session/scan/sub-request context,
with ambiguous attribution left unresolved. Global request, deadline, concurrency,
retained-response and report-send limits are shared across clients. Optional
report sends are serial and stop invocation-wide after a 429; missing examples
remain supplemental. Typed responses and transient secret discovery precede
allowlisted persistence. Final shared projection occurs after all app report reads,
so a later DLP match can withhold an earlier long excerpt before truncation.

Named bulk submissions persist version 2 with name/revision/app/endpoint metadata;
legacy jobs retain version 1. State validation rejects malformed version/reference
combinations and endpoint inconsistencies. Resume resolves only the saved binding,
refuses removal or app/endpoint changes before a request, and warns/checkpoints
explicit same-app rotation before polling with that one current credential.
Accepted submissions are not resubmitted. The revision is not derived from the
secret and does not claim post-rotation service qualification.

## Independent verification so far

`/tmp/airs-a1-independent-focus.log` records eight passing routing tests, including
guarded two-key ownership, unbound management retention, global rate-limit stop,
later-app DLP protection, unused-key preview masking, direct no-fallback behavior,
exact-name ambiguity, named-only doctor and bulk rotation/rebinding refusal.
Five existing scanner-management tests pass, covering snapshots, read views,
duplicate bindings, private input, restore in both formats and scalar preservation.
The new repeated-options regression is the sole failing case in that run and
reproduces blocking finding 1. No final acceptance score is implied until that
regression and the old-native compatibility gate are resolved.

## Follow-up review

The repetition guard was extended across named local flags and new direct scanner
selectors. Independent reruns now pass all six local scanner tests and all eight
routing tests. Four scanner-model, two atomic-snapshot and one named-state unit
tests also pass. An independent five-step frozen-native probe verified the stated
baseline SHA-256, successful old-format read, explicit upgrade, read-only old-reader
refusal, exact-byte restore and restored old-reader access in a private synthetic
store. Findings 1 and 2 are resolved.

Score remains **8/10** pending one additional privacy finding. Registering secrets
does not itself sanitize output: new scanner-command store errors propagate to
`lib::finish` without projection, and doctor renders its check detail/hint strings
directly. Independently reproduced cases:

- A valid `tenant scanners set --stdin` input whose fake apiKey equals the synthetic
  config-directory basename, with a registered but missing tenant file, exits 1
  and echoes that registered input key in its configuration-path error.
- A valid named-only synthetic config whose fake key equals the config-directory
  basename makes doctor correctly skip the scanner probe, but its JSON stdout
  still echoes the registered key in its filesystem check details (exit 0).

Require consistent secret/control projection on new command errors and transaction
warnings, and on named-config doctor detail/hint output in every format. Add focused
canary regressions. Existing fixed texts and counts need no secret-derived content;
no secret should be echoed because it also appears in a path. The review used only
synthetic files and skipped network clients in both probes.

A further configuration correctness defect was independently reproduced while
checking admitted endpoint round trips. `canonical_endpoint` originally bounded
only the raw URL input, then returned URL-canonicalized text without checking the
expanded byte length. A synthetic HTTPS endpoint with 1,000 repeated non-ASCII
path characters is 2,024 input bytes, but canonical percent encoding exceeds
4,096 bytes. `tenant scanners set --stdin` succeeded, then the immediately
subsequent scanner-list read refused the newly written config. Reject an admitted
canonical URL exceeding the same schema bound before any mutation, including
bind-default, and pin unchanged live/snapshot bytes on rejection. This probe made
no HTTP call. Acceptance remains pending this fix as well as complete output
projection for named credential errors, doctor and bulk/resume previews/status.

## Final review after remediation

Final code-review score: **9/10 — accepted**.

All identified implementation blockers are resolved:

- New named-command scalar flags and direct `--scanner` selectors are checked
  before input/store access. The guard consumes option values so flag-shaped
  profile/path text is not counted as another option, while repeatable scan/report
  IDs retain their intended behavior.
- The frozen pre-A1 native reader qualification was independently repeated using
  the SHA-256-verified RC3 artifact. Old scalar reads, named-table refusal,
  private exact-byte snapshots, explicit restore and restored old-reader access
  all passed with reader bytes unchanged and no API calls.
- Final invocation failures and dynamic interrupt contexts now pass through the
  complete invocation SecretSet after signal handling. This covers local scanner,
  direct and bulk errors without changing their exit codes. Doctor projects detail
  and hint text before every renderer; tenant lock/durability warnings and bulk/
  resume previews, summaries, state-path notices and CSV durability warnings use
  the same secret registry. The original input-error, doctor and bulk-error canary
  probes now pass independently with no credential echo.
- Canonical admitted endpoints are bounded after percent encoding as well as
  before parsing. The original non-ASCII expansion probe now fails as usage before
  mutation; existing live config and snapshot state remain unchanged and readable.

The permanent cases additionally cover named apiToken on the wire (Bearer only,
no flat key or management headers), explicit bind-default key/token selection,
absent-kind refusal, and successful bulk/resume output in JSON/pretty/table with
an unused named secret coinciding with profile and paths. The latest targeted
root log records all **11 routing and 9 local scanner tests passing**. Independent
runs also passed four scanner-model, two snapshot and one named-state tests; the
final independent zero-network probes cover all three doctor formats, registered
input-path error withholding, existing bulk-output error withholding, canonical
endpoint rejection and repeated-option refusal.

No remaining A1 code blocker was found. This score concerns the A1 implementation
scope. The root must still finish the required full format/lint/workspace/parity
checks before marking the phase complete. Full extracted-package and service
qualification remain later gates; these tests do not establish remote credential
ownership or historical access after rotation.

### Scoped final-error refinement

Re-reviewed the final compatibility refinement: **9/10 remains accepted**.
The new final local-error layer is enabled by presence of a named scanner namespace
(including its explicit empty marker), or by scanner-management entry before input
is consumed. Its in-memory flag shares the same Arc/Mutex lifetime as SecretSet
clones; matcher updates do not reset it, and it is never serialized into credentials
or job state. Final error and dynamic interruption projection still occurs after
signal-context selection. Pure legacy invocations retain their earlier final-error
behavior, avoiding unrelated short inline-key coincidences in fixed gateway errors.
DESIGN §§3.15 and 6 now state this boundary explicitly.

Independent zero-network probes of both activation paths still pass: a submitted
scanner input key is withheld from a later missing-config-path error before any
namespace exists, and a loaded named namespace withholds its key from a bulk
existing-output error while preserving that output. No new A1 safety or routing
blocker was found. Required full-suite completion remains the root's phase gate.
