# Bulk Async Scan Rewrite (Scan/Report Retrieval Deferred)

Date: 2026-07-08 (reliability correction: 2026-07-17)
Status: Approved for bulk-scan reliability; scan/report retrieval is deferred

## Summary

This correction makes the bulk-scan requirements normative. The retrieval design is retained for
future work, but it is not implemented or accepted as part of this change:

1. **Deferred: retrieve scan results and threat reports by ID** — a proposed
   `airs runtime get-scan <id...>` command. No such command is included in this release.
2. **Rewrite the broken bulk async scan** — preserve every prompt and detection, correlate results
   by stable request identity, checkpoint submission outcomes safely, project output atomically, and
   make `resume-poll` idempotent wherever the AIRS API provides a definite outcome.

## Background / Current State

SDK: `@cdot65/prisma-airs-sdk` **0.13.2 or newer**. `Scanner` exposes:

- `syncScan(aiProfile, content, opts?)` → `ScanResponse`
- `asyncScan(scanObjects: AsyncScanObject[], opts?)` → `AsyncScanResponse` — **1–20 objects per call, returns ONE `scan_id` (+ optional `report_id`) for the whole submission**
- `queryByScanIds(scanIds: string[], opts?)` → `ScanIdResult[]` — 1–5 scan-ids per call
- `queryByReportIds(reportIds: string[], opts?)` → `ThreatScanReport[]` — 1–5 report-ids per call

The per-call `ScanCallOptions` supports `numRetries`. SDK exceptions expose structured
`failureKind`, `statusCode`, and normalized `retryAfterMs` metadata. Those contracts are required
to distinguish a definite rejection from a POST whose acceptance is unknown.

Hard limits: `MAX_NUMBER_OF_BATCH_SCAN_OBJECTS = 20`; `MAX_NUMBER_OF_SCAN_IDS = 5`;
`MAX_NUMBER_OF_REPORT_IDS = 5`.

Key shapes:

- `ScanIdResult` = `{ source?, req_id?: number, status?, scan_id?, result?: ScanResponse, [k]: unknown }`
- `ScanResponse` = `{ report_id, scan_id, category, action, timeout, error, errors[], prompt_detected?: { url_cats, dlp, injection, toxic_content, malicious_code, source_code, agent, topic_violation }, response_detected?... }`
- `ThreatScanReport` = `{ report_id?, scan_id?, req_id?: number, transaction_id?, detection_results?[]: { detection_service, verdict, action, ... } }`
- `RuntimeScanResult` = the backward-compatible sync/legacy shape with `action: allow|block`
- `BulkScanResult` = the correlated async shape with `action: allow|block|failed`, plus
  `index` and `reqId`

**The original core bug:** the old `submitBulkScan` treated each submission-level `scan_id` as if
it represented one prompt. The old `pollResults` then returned one result per `scan_id` and the CLI
reattached prompts by array position. For N>5 prompts, four prompts per submission were dropped and
later rows were misaligned. It also discarded the `prompt_detected` data present in each nested
`ScanResponse`.

The old `resume-poll` could not reattach prompts because state persisted only scan IDs and a count.
The corrected model persists each prompt and its complete submission/result lifecycle.

---

## Feature 1 — Retrieve scan/report by ID

**Status: deferred.** The following section is a future proposal, not current CLI behavior or an
acceptance criterion for the bulk reliability work.

### Command

`airs runtime get-scan <id...>` — accepts 1..N IDs, `--output pretty|json|yaml` (default pretty).

### Routing

Classify each ID:

- Matches UUID v4 shape → **scan-id** → `queryByScanIds`
- Starts with `R` (report-id convention) → **report-id** → `queryByReportIds`
- Otherwise → usage error (exit 2)

IDs are grouped by type; each group is chunked into ≤5 and dispatched to its SDK method. Mixed scan-ids and report-ids in one invocation are allowed — each group renders under its own heading. If total of either group exceeds 5, chunk into multiple calls.

### Service (`SdkRuntimeService`, `src/airs/runtime.ts`)

Add thin wrappers:

```ts
getScanResults(scanIds: string[]): Promise<ScanIdResult[]>   // chunks by 5, concats
getScanReports(reportIds: string[]): Promise<ThreatScanReport[]>  // chunks by 5, concats
```

When this feature is implemented, expose these methods without breaking the existing
`RuntimeService` interface.

### Renderer (`src/cli/renderer/runtime.ts`)

- `renderScanIdResult(result)` — scan_id, report_id, status, category, action, and a per-detection table from `result.result.prompt_detected` (topic_violation, injection, toxic_content, dlp, url_cats, malicious_code, source_code, agent).
- `renderThreatReport(report)` — report_id, scan_id, req_id, then a table over `detection_results[]` (detection_service, verdict, action).
- Discipline: data to stdout, decorative/status to stderr; `--output json|yaml` emits the raw SDK object(s) so `| jq` parses. For multiple IDs, `json` emits an array.

### Errors

Unknown/expired IDs: the SDK returns empty or errors — surface a clear "no result for `<id>`" without aborting the whole batch; nonzero exit only if every requested ID failed.

---

## Feature 2 — Bulk Async Scan Rewrite

### Command

`airs runtime bulk-scan --profile <name> --file <file> [--output-file <file>] [--session-id <id>] [--batch-size <n>]`

- `--batch-size` default **25**, min 1. Internally each batch is split into `ceil(batchSize/5)` `asyncScan` calls.
- Existing deprecated aliases (`--input`→`--file`, `--output`→`--output-file`) preserved.
- `--batch-size` must be a strict positive safe integer. Fractional, zero, negative, or mixed-text
  values are usage errors.

### Constants (`src/airs/runtime.ts`)

Use named limits rather than implicit chunk sizes:

```ts
const SDK_ASYNC_MAX = 20;          // SDK per-call object cap
const SDK_QUERY_MAX = 5;           // SDK per-call scan-id cap
const DEFAULT_BATCH_SIZE = 25;     // prompts per sequential batch
```

Polling also has a finite no-progress limit (default 120 polls). A scan that never reaches a
terminal state must fail operationally rather than run forever.

### Data model

Each prompt receives a stable global `req_id` equal to its zero-based input index. One SDK receipt
maps every prompt in that 1–20 item submission to the same `scan_id` while retaining distinct
`req_id` values:

```ts
interface BatchEntry { scanId: string; reqId: number; index: number; prompt: string }
interface SubmittedBatch { scanId: string; reportId?: string; entries: BatchEntry[] }
interface BulkScanResult extends Omit<RuntimeScanResult, 'action'> {
  index: number;
  reqId: number;
  action: 'allow' | 'block' | 'failed';
}

type BulkScanItemStatus =
  | 'pending'
  | 'submitting'
  | 'submitted'
  | 'complete'
  | 'failed'
  | 'ambiguous';

interface BulkScanItemState {
  index: number;
  reqId: number;
  prompt: string;
  status: BulkScanItemStatus;
  scanId?: string;
  receiptReportId?: string;
  result?: BulkScanResult;
  error?: string;
}
```

`reqId === index`; indices are unique and contiguous from zero. A submitted item requires a
`scanId`; complete/failed items require a correlated result.

### Flow (orchestrated in the CLI command)

```
prompts = parseInputFile(...)
state = one pending item per prompt
atomically save state BEFORE any POST
acquire the per-state owner lock
atomically write the empty CSV projection
for each contiguous slice of `batchSize` prompts:
  for each 1–20 prompt SDK chunk:
    mark items submitting; atomically save state
    receipt = submitBatch(profile, chunk, sessionId)
    mark items submitted with scan/report receipt; atomically save state
  poll every accepted receipt in this logical batch
  record correlated complete/failed results; atomically save state
  atomically replace CSV with the full ordered projection of all stored results
print summary (stderr)
```

The next logical batch is not submitted until all accepted receipts in the current logical batch
have been polled. The `submitting` checkpoint closes the crash window as far as possible without a
server-side idempotency key.

### Service methods (`SdkRuntimeService`)

The reliable path uses:

- `submitBatch(profileName, prompts: {index:number; prompt:string}[], sessionId?)` → `SubmittedBatch`
  - Requires exactly 1–20 prompts and makes exactly one `asyncScan` call per attempt. The CLI, not
    this method, splits logical batches.
  - Passes `{ numRetries: 0 }` to the SDK. This prevents a hidden SDK replay of a non-idempotent
    POST and makes the CLI's submission state machine the only retry owner.
- `pollBatch(batch: SubmittedBatch, intervalMs, retryOpts?)` → `BulkScanResult[]` (ordered by `index`)
  - Polls the receipt until every entry resolves (complete or failed), with SDK retries disabled.
  - Maps each `ScanIdResult` to its entry by `(scan_id, req_id)`.
  - Builds results from the nested `ScanResponse`, preserving `allow` and `block`, and
    computes `triggered` as the OR of all eight detector flags.
  - Converts terminal failed/timeout or unknown-action responses to `action: failed`,
    `category: error`, and a useful `error` string. An operational failure must never appear as
    `allow`; no fourth runtime action is emitted.
  - Stops after the configured consecutive no-progress limit.

The ordered detector set is `topic_violation`, `injection`, `toxic_content`, `dlp`, `url_cats`,
`malicious_code`, `source_code`, and `agent`. Bulk scanning is a general firewall check, not the
topic-only signal used by the guardrail optimization loop.

### Submission outcome and retry policy

The CLI may retry a POST only when SDK metadata proves the attempt received an HTTP 429 response:
`failureKind === 'http' && statusCode === 429`. It honors a non-negative `retryAfterMs`; otherwise
it uses bounded exponential backoff (five retries by default). Matching text such as "429" in a
network error is not proof of rejection and must not trigger a replay.

After retries finish or another error occurs:

- A definite HTTP 4xx response means the service rejected the submission. Items return to
  `pending` and may be safely attempted by `resume-poll` after the underlying problem is fixed.
- A network error, timeout before a response, HTTP 5xx, process death while `submitting`, or any
  other unknown outcome is `ambiguous`: AIRS may have accepted the POST. Resume must not submit it
  again automatically.
- An accepted receipt is checkpointed as `submitted` before polling and is never resubmitted.

Exactly-once submission is impossible without server-side idempotency or a reconciliation API.
The safe local behavior is at-most-once for ambiguous outcomes, with explicit operator review.

### Tolerant mapping (KEY RISK — verify first)

Live verification confirmed that `queryByScanIds` may return multiple rows for one `scan_id`, in
an order unrelated to input order. The primary key is therefore `(scan_id, req_id)`, never array
position or `scan_id` alone. The mapping layer remains tolerant of both documented shapes:

- If results carry `req_id` → map by `(scan_id, req_id)`.
- If a batch `scan_id` yields a single row without per-`req_id` breakdown → fall back to `queryByReportIds(report_id)` (returns `ThreatScanReport` per `req_id`) to recover per-prompt detail.

Report fallback validates scan/report identity before accepting a row. Any unresolvable or
conflicting identity fails closed instead of guessing which prompt owns the result.

### CSV (atomic `formatResultsCsv` projection)

The normative columns are:

```
prompt,action,category,triggered,topic_violation,injection,toxic_content,dlp,url_cats,malicious_code,source_code,agent,scan_id,report_id,error
```

`formatResultsCsv` builds a deterministic, input-ordered projection. After each completed SDK
receipt, the command writes the entire projection to a mode-`0600` temporary file and atomically
renames it over the destination. It does not append. Repeated resume is therefore byte-identical
and cannot duplicate rows. Booleans render as `true`/`false`; every field is CSV-quoted and embedded
quotes are doubled.

### State (`src/cli/bulk-scan-state.ts`)

New shape (versioned for tolerance):

```ts
interface BulkScanState {
  version: 2;
  profile: string;
  sessionId?: string;
  outputFile: string;
  batchSize: number;
  createdAt: string;
  updatedAt: string;
  items: BulkScanItemState[];
}
```

- State is schema-validated on load, including lifecycle-dependent receipt/result requirements and
  unique contiguous indices. Malformed or legacy files are rejected with a clear error.
- State is atomically replaced before the first POST, immediately before each POST, after every
  receipt, and whenever each newly terminal prompt result is observed.
- The default state directory is mode `0700`; state files are mode `0600`. State contains raw prompt
  text and must be treated as sensitive data.
- Each active bulk/resume invocation holds an adjacent owner lock for the job lifetime. A second
  invocation exits before submitting. A lock owned by a dead local process is removed and retried;
  malformed/unverifiable ownership requires explicit operator review.

### resume-poll

`airs runtime resume-poll <stateFile> [--output-file <file>]`

- Loads and validates v2 state; default output is `state.outputFile`, overridable by `--output-file`.
- Polls known `submitted` receipts and restores exact prompt correlation from persisted items.
- Resubmits only `pending` items, in the original logical `batchSize` boundaries and 1–20 item SDK
  chunks. Complete/failed/submitted items are never submitted again.
- If any item is `submitting` or `ambiguous`, it first preserves results for all known accepted
  receipts, then stops with an actionable error. It does not guess or auto-resubmit.
- Atomically rebuilds the full CSV from state whenever a newly terminal prompt result is
  checkpointed; repeated resume does not duplicate output.

### Interface drift fix (`src/airs/types.ts`)

Keep the existing `RuntimeService` and `RuntimeScanResult` contracts backward compatible. Add a
`ReliableRuntimeService` extension for `submitBatch`/`pollBatch`, including `sessionId` and retry
options where used, and use `BulkScanResult` for the three terminal bulk actions. Retrieval methods
remain deferred. `batchSize` is CLI orchestration state, not an SDK service parameter.

---

## Testing

- Public Commander seam: seven prompts spanning two SDK receipts produce seven input-ordered rows,
  even when AIRS returns `req_id` rows out of order.
- `(scan_id, req_id)` → prompt mapping, including N>5 alignment and shared scan/report IDs.
- Tolerant mapping fallback to `queryByReportIds` when `req_id` absent.
- Eight-way `triggered` aggregation; `source_code`, `agent`, timeout, and failed responses.
- CSV projection, ordering, quoting, operational error column, atomic replacement, and byte-identical
  repeated resume.
- State v2 validation, atomic save/load round-trip, permissions, malformed input, and legacy v1
  rejection.
- POST calls pass SDK retries 0; confirmed 429 retries honor Retry-After; network errors containing
  "429" are not replayed; final definite 4xx remains pending; unknown outcomes become ambiguous.
- Resume never resubmits accepted or ambiguous items and safely retries only pending items.
- Concurrent resume attempts produce at most one submitter; dead-owner locks are recoverable.
- Polling rate-limit metadata and finite no-progress termination.
- Partial success is preserved and any error result produces exit code 1.
- Strict `--batch-size` validation and sequential logical-batch behavior.
- Deferred retrieval work will require `get-scan` ID classification (UUID vs `R`-prefix vs invalid)
  and both render paths; these are not acceptance criteria for this change.

## Out of Scope

- No change to `syncScan` / `runtime scan`.
- No `runtime get-scan` command or scan/report retrieval service methods in this change.
- No red-team retrieval changes (already has by-ID commands).
- No promise of exactly-once POST execution without an AIRS idempotency/reconciliation contract.

## Open Questions / Risks

1. **Ambiguous POST outcome** — no local state machine can prove whether AIRS accepted a request
   when the connection fails before a response. The CLI fails closed and requires operator review;
   a future AIRS idempotency key or receipt-reconciliation endpoint would remove this limitation.
2. **Report fallback fidelity** — `queryByReportIds` may expose detector-oriented details rather than
   the complete nested `ScanResponse`; aliases are normalized and identity is validated, but direct
   `(scan_id, req_id)` rows remain preferred.
3. **Prompt sensitivity** — resume correctness requires storing prompt text. Restrictive filesystem
   permissions reduce accidental exposure but do not replace host disk encryption and access
   controls.
