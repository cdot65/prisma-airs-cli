# Bulk Async Scan Rewrite + Scan/Report Retrieval by ID

Date: 2026-07-08
Status: Approved (design)

## Summary

Two features for the runtime scanning surface:

1. **Retrieve scan results and threat reports by ID** — a new `airs runtime get-scan <id...>` command that fetches structured scan/report data from the AIRS API by scan-id or report-id, rendered for humans and agents alike.
2. **Rewrite the broken bulk async scan** — `airs runtime bulk-scan` currently drops 4 of every 5 prompts, misaligns output rows, and discards detection data. Rewrite it to process sequential batches, map results back per-prompt with full detection data, write output incrementally, and make `resume-poll` actually restore prompts.

## Background / Current State

SDK: `@cdot65/prisma-airs-sdk` `^0.12.0`. `Scanner` (`node_modules/@cdot65/prisma-airs-sdk/dist/index.d.ts:31241`) exposes:

- `syncScan(aiProfile, content, opts?)` → `ScanResponse`
- `asyncScan(scanObjects: AsyncScanObject[])` → `AsyncScanResponse` — **1–5 objects per call, returns ONE `scan_id` (+ optional `report_id`) for the whole batch**
- `queryByScanIds(scanIds: string[])` → `ScanIdResult[]` — 1–5 scan-ids per call
- `queryByReportIds(reportIds: string[])` → `ThreatScanReport[]` — 1–5 report-ids per call

Hard limits (all = 5): `MAX_NUMBER_OF_BATCH_SCAN_OBJECTS`, `MAX_NUMBER_OF_SCAN_IDS`, `MAX_NUMBER_OF_REPORT_IDS`.

Key shapes:

- `ScanIdResult` = `{ source?, req_id?: number, status?, scan_id?, result?: ScanResponse, [k]: unknown }`
- `ScanResponse` = `{ report_id, scan_id, category, action, timeout, error, errors[], prompt_detected?: { url_cats, dlp, injection, toxic_content, malicious_code, agent, topic_violation }, response_detected?... }`
- `ThreatScanReport` = `{ report_id?, scan_id?, req_id?: number, transaction_id?, detection_results?[]: { detection_service, verdict, action, ... } }`
- `RuntimeScanResult` (`src/airs/types.ts:45-54`) = `{ prompt, response?, scanId, reportId, action, category, triggered, detections: Record<string,boolean> }`

**The core bug:** `submitBulkScan` (`src/airs/runtime.ts:70-93`) treats each batch-level `scan_id` as if it were one prompt. `pollResults` returns one result per `scan_id`; the CLI then reattaches prompts by index (`runtime.ts:237-240`), so for N>5 prompts, result[i] is paired with prompt[i] even though there are only `ceil(N/5)` results. Prompts 2–5 of every batch are silently dropped and remaining rows misalign. Separately, `processQueryResults` (`runtime.ts:160-196`) hardcodes `triggered:false` / `detections:{}` under the false belief that detection data is unavailable from the async path — but each `ScanIdResult.result` is a full `ScanResponse` with `prompt_detected`.

`resume-poll` (`runtime.ts:704-754`) never reattaches prompts because `BulkScanState` (`src/cli/bulk-scan-state.ts:4-10`) persists only `scanIds` + a count, not the prompts.

---

## Feature 1 — Retrieve scan/report by ID

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

Extend the `RuntimeService` interface in `src/airs/types.ts` with both.

### Renderer (`src/cli/renderer/runtime.ts`)

- `renderScanIdResult(result)` — scan_id, report_id, status, category, action, and a per-detection table from `result.result.prompt_detected` (topic_violation, injection, toxic_content, dlp, url_cats, malicious_code, agent).
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

### Constants (`src/airs/runtime.ts`)

Replace hardcoded `5`/`25`:

```ts
const SDK_ASYNC_MAX = 5;           // SDK per-call object cap
const SDK_QUERY_MAX = 5;           // SDK per-call scan-id cap
const DEFAULT_BATCH_SIZE = 25;     // prompts per sequential batch
```

Reuse existing `DEFAULT_POLL_INTERVAL_MS`, `DEFAULT_MAX_RETRIES`, `DEFAULT_BASE_DELAY_MS` and the current rate-limit backoff logic.

### Data model

Per prompt, a unique global `req_id` (its 0-based index in the full input). A submitted batch tracks the mapping:

```ts
interface BatchEntry { scanId: string; reqId: number; index: number; prompt: string }
interface SubmittedBatch { scanIds: string[]; entries: BatchEntry[]; done: boolean }
```

### Flow (orchestrated in the CLI command)

```
prompts = parseInputFile(...)
write CSV header (once)
for each contiguous slice of `batchSize` prompts:
  batch = submitBatch(profile, slice, sessionId)     // service
  state.batches.push(batch); saveBulkScanState(...)   // crash-safe, BEFORE polling
  results = pollBatch(batch, intervalMs, retryOpts)    // service; ordered by index
  appendCsvRows(outputFile, results)                   // incremental
  batch.done = true; saveBulkScanState(...)
  update blocked/allowed counters
print summary (stderr)
```

### Service methods (`SdkRuntimeService`)

Replace `submitBulkScan`/`pollResults`/`processQueryResults` with:

- `submitBatch(profileName, prompts: {index:number; prompt:string}[], sessionId?)` → `SubmittedBatch`
  - Chunks into ≤`SDK_ASYNC_MAX` `AsyncScanObject`s (`req_id = index`), one `asyncScan` per chunk, collects `scan_id`s, builds `entries`.
- `pollBatch(batch: SubmittedBatch, intervalMs, retryOpts?)` → `RuntimeScanResult[]` (ordered by `index`)
  - Polls `batch.scanIds` (≤`SDK_QUERY_MAX`/query) until every entry resolves (complete or failed).
  - Maps each `ScanIdResult` to its entry by `(scan_id, req_id)`.
  - Builds `RuntimeScanResult` from `result.result` (full `prompt_detected` → `detections` + `triggered = OR of the 6 detection flags`, matching `scanPrompt`'s aggregation).

`triggered` for bulk uses the same 6-way OR as `scanPrompt` (`topic_violation || injection || toxic_content || dlp || url_cats || malicious_code`) — bulk scan is a general firewall check, not topic-only.

### Tolerant mapping (KEY RISK — verify first)

Design assumes `queryByScanIds` returns one `ScanIdResult` per `(scan_id, req_id)` (schema has `req_id`). **Before building on it, verify against a live or captured async response.** The mapping layer is written tolerant of both shapes:

- If results carry `req_id` → map by `(scan_id, req_id)`.
- If a batch `scan_id` yields a single row without per-`req_id` breakdown → fall back to `queryByReportIds(report_id)` (returns `ThreatScanReport` per `req_id`) to recover per-prompt detail.

This tolerance is a small dispatch in `pollBatch`, isolated and unit-tested.

### CSV (`formatResultsCsv` + incremental helpers)

Columns:

```
prompt,action,category,triggered,topic_violation,injection,toxic_content,dlp,url_cats,malicious_code,scan_id,report_id
```

- Add `csvHeader()` and `csvRow(result)` helpers; `formatResultsCsv` composes them (still used by `resume-poll` for full writes and by tests).
- Command writes header once, appends rows per batch (crash-safe partial output). Booleans render as `true`/`false`; prompt double-quote-escaped.

### State (`src/cli/bulk-scan-state.ts`)

New shape (versioned for tolerance):

```ts
interface BulkScanState {
  version: 2;
  profile: string;
  sessionId?: string;
  outputFile: string;
  batchSize: number;
  timestamp?: string;
  batches: SubmittedBatch[];   // each with entries (scanId,reqId,index,prompt) + done flag
}
```

- `saveBulkScanState` writes after each submit and after each batch completes.
- `loadBulkScanState` tolerant of legacy v1 (`{scanIds, promptCount}`): loads with empty prompts, warns that resumed rows will lack prompt text.

### resume-poll

`airs runtime resume-poll <stateFile> [--output-file <file>]`

- Loads state; default output = `state.outputFile`, overridable by `--output-file`.
- Writes header, iterates `batches` where `!done`, calls `pollBatch`, appends full rows (prompt column populated from persisted `entries`), marks done, re-saves.

### Interface drift fix (`src/airs/types.ts`)

Update `RuntimeService` to match the real signatures: add `sessionId`, `retryOpts`, `batchSize` where used; declare `submitBatch`/`pollBatch`/`getScanResults`/`getScanReports`.

---

## Testing (service-layer; `src/cli/**` is coverage-excluded)

- `(scan_id, req_id)` → prompt mapping, including multi-batch (N>5) alignment (the exact regression).
- Tolerant mapping fallback to `queryByReportIds` when `req_id` absent.
- `triggered` 6-way OR aggregation from `prompt_detected`.
- CSV header + row formatting (booleans, quoting, escaping).
- State v2 save/load round-trip; legacy v1 tolerance + warning.
- `get-scan` ID classification (UUID vs `R`-prefix vs invalid) and both render paths.
- Rate-limit backoff preserved (existing behavior unchanged).

## Out of Scope

- No change to `syncScan` / `runtime scan`.
- No new detection types beyond the existing 6.
- No red-team retrieval changes (already has by-ID commands).

## Open Questions / Risks

1. **`queryByScanIds` result granularity** — per-`req_id` vs batch-level. Mitigated by tolerant mapping; verify against a real async response early in implementation.
2. **`asyncScan` `report_id` presence** — `report_id` is optional on `AsyncScanResponse`; the per-prompt `report_id` is taken from each `ScanResponse` (`result.result.report_id`), which is required there, so the CSV `report_id` column is populated from the query result, not the submit response.
