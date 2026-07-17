# Bulk Async Scan Rewrite + Scan/Report Retrieval — Implementation Plan

> [!WARNING]
> **Historical plan — superseded.** This task-by-task plan preserves the original implementation
> history, but its batch-centric state, append-only CSV, six-detector list, fail-open error handling,
> and retry assumptions are unsafe and no longer normative. Use the corrected approved design in
> `docs/superpowers/specs/2026-07-08-bulk-scan-and-scan-retrieval-design.md` and the current tests/code
> as the source of truth. In particular, SDK 0.13.2 accepts 20 async objects and current bulk
> actions are `allow`, `block`, or `failed`. Do not execute the snippets below verbatim.

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix the broken `runtime bulk-scan` (correct per-prompt mapping, full detection data, incremental output, resumable) and add `runtime get-scan <id...>` to retrieve scan results / threat reports by ID.

**Architecture:** All SDK access stays in `SdkRuntimeService` (`src/airs/runtime.ts`). Bulk scan is decomposed into pure, unit-testable helpers (`scanResponseToResult`, `threatReportToResult`, `entryKey`) plus two service methods (`submitBatch`, `pollBatch`) and two retrieval wrappers (`getScanResults`, `getScanReports`). The CLI (`src/cli/commands/runtime.ts`) orchestrates sequential batches and incremental CSV writes; rich resumable state lives in `src/cli/bulk-scan-state.ts`; rendering lives in `src/cli/renderer/runtime.ts`.

**Tech Stack:** TypeScript (ESM, `.js` import specifiers), vitest, commander, `@cdot65/prisma-airs-sdk` `^0.12.0`, Biome (single quotes, semicolons, 2-space indent, 100-char width).

**Spec:** `docs/superpowers/specs/2026-07-08-bulk-scan-and-scan-retrieval-design.md`

---

## Key SDK facts (do not re-derive)

- `Scanner.asyncScan(objs)` — accepts **1–5** `AsyncScanObject` `{ req_id:number, scan_req:{ ai_profile, contents:[{prompt}], session_id? } }`; returns **one** `{ received, scan_id, report_id? }` for the whole batch.
- `Scanner.queryByScanIds(ids)` — 1–5 scan-ids → `ScanIdResult[]` = `{ req_id?:number, status?:string, scan_id?:string, result?:ScanResponse }`.
- `Scanner.queryByReportIds(ids)` — 1–5 report-ids → `ThreatScanReport[]` = `{ report_id?, scan_id?, req_id?:number, detection_results?:[{ detection_service, verdict, action }] }`.
- `ScanResponse` = `{ scan_id, report_id, category, action, prompt_detected?:{ topic_violation, injection, toxic_content, dlp, url_cats, malicious_code, agent } , ... }`.
- SDK hard-caps every batch at 5 (`MAX_NUMBER_OF_*`).

## File map

- **Modify** `src/airs/types.ts` — add `IndexedPrompt`, `BatchEntry`, `SubmittedBatch`; re-export SDK `ScanIdResult`/`ThreatScanReport`; update `RuntimeService` interface.
- **Modify** `src/airs/runtime.ts` — detection-key constant, pure helpers, `submitBatch`, `pollBatch`, `getScanResults`, `getScanReports`, extend CSV; remove old `submitBulkScan`/`pollResults`/`processQueryResults`.
- **Modify** `src/cli/bulk-scan-state.ts` — v2 state shape, legacy detection.
- **Modify** `src/cli/commands/runtime.ts` — rewrite `bulk-scan` (sequential batches, `--batch-size`, incremental CSV), rewrite `resume-poll`, add `get-scan`.
- **Modify** `src/cli/renderer/runtime.ts` — `renderScanIdResults`, `renderThreatReports`.
- **Test** `tests/unit/airs/runtime.spec.ts` — helpers, submit/poll, retrieval, CSV.
- **Test (new)** `tests/unit/cli/bulk-scan-state.spec.ts` — v2 round-trip, legacy rejection (create if absent).
- **Docs** `docs-site/docs/about/release-notes.md`, `.changeset/*.md`, `CLAUDE.md` runtime section.

---

## Task 1: Add shared types

**Files:**
- Modify: `src/airs/types.ts:6-10` (SDK import), `:44-64` (runtime types + `RuntimeService`)

- [ ] **Step 1: Extend the SDK type import**

In `src/airs/types.ts`, change the import block at the top (currently lines 6-10) to also pull the two SDK result types:

```ts
import type {
  CreateCustomTopicRequest,
  CreateSecurityProfileRequest,
  CustomTopic as SdkCustomTopic,
  ScanIdResult,
  ThreatScanReport,
} from '@cdot65/prisma-airs-sdk';
```

- [ ] **Step 2: Re-export the SDK result types**

Change the existing re-export line (currently line 24) to:

```ts
export type {
  CreateCustomTopicRequest,
  CreateSecurityProfileRequest,
  SdkCustomTopic,
  ScanIdResult,
  ThreatScanReport,
};
```

- [ ] **Step 3: Add bulk-scan types after `RuntimeScanResult`**

Directly after the `RuntimeScanResult` interface (ends at line 54) insert:

```ts
/** A prompt paired with its 0-based position in the full input list. */
export interface IndexedPrompt {
  index: number;
  prompt: string;
}

/** One submitted prompt within a batch: which async scan_id + req_id it maps to. */
export interface BatchEntry {
  scanId: string;
  reqId: number;
  index: number;
  prompt: string;
}

/** A submitted bulk-scan batch: the async scan_ids it produced and its per-prompt entries. */
export interface SubmittedBatch {
  scanIds: string[];
  entries: BatchEntry[];
  done: boolean;
}
```

- [ ] **Step 4: Replace the `RuntimeService` interface**

Replace the whole `RuntimeService` interface (currently lines 56-64) with:

```ts
/** Options controlling poll retry/backoff behaviour. */
export interface PollRetryOptions {
  maxRetries?: number;
  baseDelayMs?: number;
  onRetry?: (attempt: number, delayMs: number) => void;
}

/** Contract for runtime scanning operations (sync + async). */
export interface RuntimeService {
  /** Scan a single prompt (and optional response) synchronously. */
  scanPrompt(profileName: string, prompt: string, response?: string): Promise<RuntimeScanResult>;
  /** Submit one batch of indexed prompts for async scanning. */
  submitBatch(
    profileName: string,
    prompts: IndexedPrompt[],
    sessionId?: string,
  ): Promise<SubmittedBatch>;
  /** Poll a submitted batch until every entry completes; results ordered by index. */
  pollBatch(
    batch: SubmittedBatch,
    intervalMs?: number,
    retryOpts?: PollRetryOptions,
  ): Promise<RuntimeScanResult[]>;
  /** Fetch scan results by scan-id (chunked ≤5). */
  getScanResults(scanIds: string[]): Promise<ScanIdResult[]>;
  /** Fetch detailed threat reports by report-id (chunked ≤5). */
  getScanReports(reportIds: string[]): Promise<ThreatScanReport[]>;
}
```

Note: `PollRetryOptions` currently also lives in `runtime.ts` (line 9). Task 2 will import it from here instead to avoid duplication.

- [ ] **Step 5: Type-check**

Run: `pnpm tsc --noEmit`
Expected: errors ONLY in `src/airs/runtime.ts` and `src/cli/commands/runtime.ts` (they still reference the removed methods). No errors originating in `types.ts`. Those consumers are fixed in Tasks 2–5.

- [ ] **Step 6: Commit**

```bash
git add src/airs/types.ts
git commit -m "feat(runtime): add bulk-scan batch types + retrieval service contract"
```

---

## Task 2: Pure result-mapping helpers in runtime.ts

**Files:**
- Modify: `src/airs/runtime.ts:1-24` (imports/constants), refactor `scanPrompt`
- Test: `tests/unit/airs/runtime.spec.ts`

- [ ] **Step 1: Write failing tests for the helpers**

Add to `tests/unit/airs/runtime.spec.ts` a new top-level `describe` (after the existing imports; the helpers are exported from runtime.ts):

```ts
import {
  DETECTION_KEYS,
  scanResponseToResult,
  threatReportToResult,
  entryKey,
} from '../../../src/airs/runtime.js';

describe('mapping helpers', () => {
  it('DETECTION_KEYS is the 6 runtime detectors in order', () => {
    expect(DETECTION_KEYS).toEqual([
      'topic_violation',
      'injection',
      'toxic_content',
      'dlp',
      'url_cats',
      'malicious_code',
    ]);
  });

  it('scanResponseToResult maps a ScanResponse + prompt to a RuntimeScanResult', () => {
    const result = scanResponseToResult(
      {
        scan_id: 's1',
        report_id: 'R1',
        action: 'block',
        category: 'malicious',
        prompt_detected: { injection: true, dlp: false },
      },
      'hack me',
    );
    expect(result).toEqual({
      prompt: 'hack me',
      response: undefined,
      scanId: 's1',
      reportId: 'R1',
      action: 'block',
      category: 'malicious',
      triggered: true,
      detections: { injection: true, dlp: false },
    });
  });

  it('scanResponseToResult triggered=false when no detector fires', () => {
    const result = scanResponseToResult(
      { scan_id: 's2', report_id: 'R2', action: 'allow', category: 'benign', prompt_detected: {} },
      'hi',
    );
    expect(result.triggered).toBe(false);
    expect(result.action).toBe('allow');
  });

  it('threatReportToResult derives detections/action from detection_results', () => {
    const result = threatReportToResult(
      {
        scan_id: 's3',
        report_id: 'R3',
        detection_results: [
          { detection_service: 'injection', verdict: 'malicious', action: 'block' },
          { detection_service: 'dlp', verdict: 'benign', action: 'allow' },
        ],
      },
      'prompt3',
    );
    expect(result.scanId).toBe('s3');
    expect(result.reportId).toBe('R3');
    expect(result.triggered).toBe(true);
    expect(result.action).toBe('block');
    expect(result.detections).toEqual({ injection: true, dlp: false });
  });

  it('entryKey joins scanId and reqId', () => {
    expect(entryKey('abc', 4)).toBe('abc::4');
  });
});
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `pnpm test -- tests/unit/airs/runtime.spec.ts -t "mapping helpers"`
Expected: FAIL — `scanResponseToResult`/`threatReportToResult`/`entryKey`/`DETECTION_KEYS` not exported.

- [ ] **Step 3: Add the constant + helpers**

In `src/airs/runtime.ts`, replace the top of the file (lines 1-24 — the imports, the `const BATCH_SIZE...` block, the `PollRetryOptions` interface, and `isRateLimitError`) with:

```ts
import { Content, type InitOptions, init, Scanner } from '@cdot65/prisma-airs-sdk';
import type {
  IndexedPrompt,
  PollRetryOptions,
  RuntimeScanResult,
  RuntimeService,
  ScanIdResult,
  SubmittedBatch,
  ThreatScanReport,
} from './types.js';

export type { PollRetryOptions };

/** SDK per-call cap for asyncScan objects and queryByScanIds ids. */
const SDK_BATCH_MAX = 5;
/** Prompts per sequential bulk-scan batch (default; overridable via CLI --batch-size). */
export const DEFAULT_BATCH_SIZE = 25;
const DEFAULT_POLL_INTERVAL_MS = 5000;
const DEFAULT_MAX_RETRIES = 5;
const DEFAULT_BASE_DELAY_MS = 10_000;

/** The 6 runtime detectors aggregated by OR into `triggered`; also the CSV detection columns. */
export const DETECTION_KEYS = [
  'topic_violation',
  'injection',
  'toxic_content',
  'dlp',
  'url_cats',
  'malicious_code',
] as const;

function isRateLimitError(err: unknown): boolean {
  if (err instanceof Error) {
    const msg = err.message.toLowerCase();
    return msg.includes('rate limit') || msg.includes('rate_limit') || msg.includes('429');
  }
  return false;
}

/** OR the 6 detectors into a single triggered flag (matches runtime firewall semantics). */
function isTriggered(detections: Record<string, boolean>): boolean {
  return DETECTION_KEYS.some((k) => !!detections[k]);
}

/** Stable key pairing an async scan_id with a per-prompt req_id. */
export function entryKey(scanId: string, reqId: number): string {
  return `${scanId}::${reqId}`;
}

/** Map an SDK ScanResponse (+ the originating prompt) to a normalized RuntimeScanResult. */
export function scanResponseToResult(
  resp: Record<string, unknown>,
  prompt: string,
  response?: string,
): RuntimeScanResult {
  const detections = (resp.prompt_detected as Record<string, boolean> | undefined) ?? {};
  return {
    prompt,
    response,
    scanId: (resp.scan_id as string) ?? '',
    reportId: (resp.report_id as string) ?? '',
    action: resp.action === 'block' ? 'block' : 'allow',
    category: (resp.category as string) ?? 'unknown',
    triggered: isTriggered(detections),
    detections,
  };
}

/** Map an SDK ThreatScanReport (+ prompt) to a RuntimeScanResult (fallback detail path). */
export function threatReportToResult(
  report: Record<string, unknown>,
  prompt: string,
): RuntimeScanResult {
  const drs = (report.detection_results as Array<Record<string, unknown>> | undefined) ?? [];
  const detections: Record<string, boolean> = {};
  let anyBlock = false;
  for (const dr of drs) {
    const fired = dr.action === 'block' || dr.verdict === 'malicious';
    detections[(dr.detection_service as string) ?? 'unknown'] = fired;
    if (dr.action === 'block') anyBlock = true;
  }
  const triggered = Object.values(detections).some(Boolean);
  return {
    prompt,
    response: undefined,
    scanId: (report.scan_id as string) ?? '',
    reportId: (report.report_id as string) ?? '',
    action: anyBlock ? 'block' : 'allow',
    category: triggered ? 'malicious' : 'benign',
    triggered,
    detections,
  };
}
```

- [ ] **Step 4: Refactor `scanPrompt` to use the helper (DRY)**

Replace the body of `scanPrompt` (currently the block building `detected`/`triggered`/return, lines 45-67) with:

```ts
    const res = await this.scanner.syncScan({ profile_name: profileName }, content, undefined);
    return scanResponseToResult(res as Record<string, unknown>, prompt, response);
```

(Keep the lines above it that build `contentOpts` and `content`.)

- [ ] **Step 5: Run the helper + existing scanPrompt tests**

Run: `pnpm test -- tests/unit/airs/runtime.spec.ts -t "mapping helpers"`
Expected: PASS.
Run: `pnpm test -- tests/unit/airs/runtime.spec.ts -t "scanPrompt"`
Expected: PASS (the refactor preserves behaviour; existing assertions at lines 65-74 still hold).

- [ ] **Step 6: Commit**

```bash
git add src/airs/runtime.ts tests/unit/airs/runtime.spec.ts
git commit -m "refactor(runtime): extract result-mapping helpers + detection-key constant"
```

Note: `runtime.ts` will not fully type-check until Task 3 replaces the old bulk methods — that's expected; do not run the full build here.

---

## Task 3: `submitBatch` + `pollBatch` service methods

**Files:**
- Modify: `src/airs/runtime.ts` — replace `submitBulkScan`/`pollResults`/`processQueryResults`
- Test: `tests/unit/airs/runtime.spec.ts`

- [ ] **Step 1: Add `queryByReportIds` to the SDK mock**

In `tests/unit/airs/runtime.spec.ts`, extend `mockScannerInstance` (currently lines 4-8) to:

```ts
const mockScannerInstance = {
  syncScan: vi.fn(),
  asyncScan: vi.fn(),
  queryByScanIds: vi.fn(),
  queryByReportIds: vi.fn(),
};
```

- [ ] **Step 2: Write failing tests for `submitBatch`**

Add a new `describe('submitBatch', ...)`:

```ts
describe('submitBatch', () => {
  it('chunks >5 prompts into multiple asyncScan calls and maps entries by scan_id+req_id', async () => {
    mockScannerInstance.asyncScan
      .mockResolvedValueOnce({ scan_id: 'batchA', report_id: 'RA' })
      .mockResolvedValueOnce({ scan_id: 'batchB', report_id: 'RB' });

    const prompts = Array.from({ length: 7 }, (_, i) => ({ index: i, prompt: `p${i}` }));
    const batch = await service.submitBatch('prof', prompts, 'sess-1');

    // 7 prompts -> chunks of 5 + 2 -> two asyncScan calls
    expect(mockScannerInstance.asyncScan).toHaveBeenCalledTimes(2);
    // first call: 5 objects with global req_ids 0..4, session_id threaded
    expect(mockScannerInstance.asyncScan).toHaveBeenNthCalledWith(1, [
      { req_id: 0, scan_req: { ai_profile: { profile_name: 'prof' }, contents: [{ prompt: 'p0' }], session_id: 'sess-1' } },
      { req_id: 1, scan_req: { ai_profile: { profile_name: 'prof' }, contents: [{ prompt: 'p1' }], session_id: 'sess-1' } },
      { req_id: 2, scan_req: { ai_profile: { profile_name: 'prof' }, contents: [{ prompt: 'p2' }], session_id: 'sess-1' } },
      { req_id: 3, scan_req: { ai_profile: { profile_name: 'prof' }, contents: [{ prompt: 'p3' }], session_id: 'sess-1' } },
      { req_id: 4, scan_req: { ai_profile: { profile_name: 'prof' }, contents: [{ prompt: 'p4' }], session_id: 'sess-1' } },
    ]);
    expect(batch.scanIds).toEqual(['batchA', 'batchB']);
    expect(batch.done).toBe(false);
    expect(batch.entries).toHaveLength(7);
    expect(batch.entries[0]).toEqual({ scanId: 'batchA', reqId: 0, index: 0, prompt: 'p0' });
    expect(batch.entries[5]).toEqual({ scanId: 'batchB', reqId: 5, index: 5, prompt: 'p5' });
  });

  it('omits session_id when not provided', async () => {
    mockScannerInstance.asyncScan.mockResolvedValueOnce({ scan_id: 'b', report_id: 'r' });
    await service.submitBatch('prof', [{ index: 0, prompt: 'x' }]);
    expect(mockScannerInstance.asyncScan).toHaveBeenCalledWith([
      { req_id: 0, scan_req: { ai_profile: { profile_name: 'prof' }, contents: [{ prompt: 'x' }] } },
    ]);
  });
});
```

- [ ] **Step 3: Write failing tests for `pollBatch` (both mapping paths)**

Add `describe('pollBatch', ...)`:

```ts
describe('pollBatch', () => {
  const batch = {
    scanIds: ['batchA'],
    entries: [
      { scanId: 'batchA', reqId: 0, index: 0, prompt: 'p0' },
      { scanId: 'batchA', reqId: 1, index: 1, prompt: 'p1' },
    ],
    done: false,
  };

  it('maps per-req_id ScanIdResult rows back to prompts with full detections', async () => {
    mockScannerInstance.queryByScanIds.mockResolvedValueOnce([
      { scan_id: 'batchA', req_id: 0, status: 'complete', result: { scan_id: 'batchA', report_id: 'R0', action: 'block', category: 'malicious', prompt_detected: { injection: true } } },
      { scan_id: 'batchA', req_id: 1, status: 'complete', result: { scan_id: 'batchA', report_id: 'R1', action: 'allow', category: 'benign', prompt_detected: {} } },
    ]);

    const results = await service.pollBatch(batch, 0);

    expect(results).toHaveLength(2);
    expect(results[0]).not.toHaveProperty('index'); // results carry no index; array is ordered by it
    expect(results[0].prompt).toBe('p0');
    expect(results[0].action).toBe('block');
    expect(results[0].triggered).toBe(true);
    expect(results[0].detections).toEqual({ injection: true });
    expect(results[1].prompt).toBe('p1');
    expect(results[1].action).toBe('allow');
  });

  it('falls back to queryByReportIds when scan rows lack req_id', async () => {
    mockScannerInstance.queryByScanIds.mockResolvedValueOnce([
      { scan_id: 'batchA', status: 'complete', result: { scan_id: 'batchA', report_id: 'RX' } },
    ]);
    mockScannerInstance.queryByReportIds.mockResolvedValueOnce([
      { scan_id: 'batchA', report_id: 'RX', req_id: 0, detection_results: [{ detection_service: 'dlp', verdict: 'malicious', action: 'block' }] },
      { scan_id: 'batchA', report_id: 'RX', req_id: 1, detection_results: [{ detection_service: 'dlp', verdict: 'benign', action: 'allow' }] },
    ]);

    const results = await service.pollBatch(batch, 0);

    expect(mockScannerInstance.queryByReportIds).toHaveBeenCalledWith(['RX']);
    expect(results[0].prompt).toBe('p0');
    expect(results[0].triggered).toBe(true);
    expect(results[1].prompt).toBe('p1');
    expect(results[1].triggered).toBe(false);
  });

  it('marks a failed entry as category=error, action=allow', async () => {
    mockScannerInstance.queryByScanIds.mockResolvedValueOnce([
      { scan_id: 'batchA', req_id: 0, status: 'failed' },
      { scan_id: 'batchA', req_id: 1, status: 'complete', result: { scan_id: 'batchA', report_id: 'R1', action: 'allow', category: 'benign', prompt_detected: {} } },
    ]);

    const results = await service.pollBatch(batch, 0);
    expect(results[0].category).toBe('error');
    expect(results[0].action).toBe('allow');
    expect(results[0].prompt).toBe('p0');
  });
});
```

Note on the first test: `index: undefined` asserts `RuntimeScanResult` has no `index` field — results are returned ordered by `entries` index instead. Keep that assertion to lock the shape.

- [ ] **Step 4: Run to verify failure**

Run: `pnpm test -- tests/unit/airs/runtime.spec.ts -t "submitBatch"`
Run: `pnpm test -- tests/unit/airs/runtime.spec.ts -t "pollBatch"`
Expected: FAIL — methods not implemented.

- [ ] **Step 5: Implement `submitBatch` + `pollBatch`, delete old methods**

In `src/airs/runtime.ts`, delete `submitBulkScan` (lines ~70-93), the `pollResults` method + its doc comment (~95-158), and `processQueryResults` (~160-196). Replace with:

```ts
  async submitBatch(
    profileName: string,
    prompts: IndexedPrompt[],
    sessionId?: string,
  ): Promise<SubmittedBatch> {
    const entries: SubmittedBatch['entries'] = [];
    const scanIds: string[] = [];

    for (let i = 0; i < prompts.length; i += SDK_BATCH_MAX) {
      const chunk = prompts.slice(i, i + SDK_BATCH_MAX);
      const scanObjects = chunk.map(({ index, prompt }) => ({
        req_id: index,
        scan_req: {
          ai_profile: { profile_name: profileName },
          contents: [{ prompt }],
          ...(sessionId ? { session_id: sessionId } : {}),
        },
      }));
      const res = await this.scanner.asyncScan(scanObjects);
      scanIds.push(res.scan_id);
      for (const { index, prompt } of chunk) {
        entries.push({ scanId: res.scan_id, reqId: index, index, prompt });
      }
    }

    return { scanIds, entries, done: false };
  }

  async pollBatch(
    batch: SubmittedBatch,
    intervalMs = DEFAULT_POLL_INTERVAL_MS,
    retryOpts?: PollRetryOptions,
  ): Promise<RuntimeScanResult[]> {
    const maxRetries = retryOpts?.maxRetries ?? DEFAULT_MAX_RETRIES;
    const baseDelay = retryOpts?.baseDelayMs ?? DEFAULT_BASE_DELAY_MS;

    const byKey = new Map<string, (typeof batch.entries)[number]>();
    for (const e of batch.entries) byKey.set(entryKey(e.scanId, e.reqId), e);

    const resolved = new Map<string, RuntimeScanResult>(); // entryKey -> result
    const uniqueScanIds = [...new Set(batch.scanIds)];
    let retryLevel = 0;

    while (resolved.size < batch.entries.length) {
      // scan_ids that still have at least one unresolved entry
      const pendingScanIds = uniqueScanIds.filter((sid) =>
        batch.entries.some((e) => e.scanId === sid && !resolved.has(entryKey(e.scanId, e.reqId))),
      );
      let sweepCompleted = true;

      for (let b = 0; b < pendingScanIds.length; b += SDK_BATCH_MAX) {
        const ids = pendingScanIds.slice(b, b + SDK_BATCH_MAX);
        let rows: Array<Record<string, unknown>>;
        try {
          rows = (await this.scanner.queryByScanIds(ids)) as Array<Record<string, unknown>>;
        } catch (err) {
          if (isRateLimitError(err) && retryLevel < maxRetries) {
            retryLevel++;
            const delayMs = baseDelay * 2 ** (retryLevel - 1);
            retryOpts?.onRetry?.(retryLevel, delayMs);
            await new Promise((r) => setTimeout(r, delayMs));
            sweepCompleted = false;
            break;
          }
          throw err;
        }

        await this.resolveRows(rows, byKey, resolved);

        if (b + SDK_BATCH_MAX < pendingScanIds.length) {
          const batchDelay = retryLevel > 0 ? baseDelay : Math.min(baseDelay, 1000);
          await new Promise((r) => setTimeout(r, batchDelay));
        }
      }

      if (sweepCompleted && retryLevel > 0) retryLevel = Math.max(0, retryLevel - 1);

      if (resolved.size < batch.entries.length) {
        const sweepDelay = retryLevel > 0 ? baseDelay * 2 ** retryLevel : intervalMs;
        await new Promise((r) => setTimeout(r, sweepDelay));
      }
    }

    // Return ordered by original prompt index.
    return [...batch.entries]
      .sort((a, b) => a.index - b.index)
      .map((e) => resolved.get(entryKey(e.scanId, e.reqId)) as RuntimeScanResult);
  }

  /**
   * Resolve query rows into per-entry results.
   * Primary path: rows carry `req_id` -> map directly by (scan_id, req_id).
   * Fallback: a terminal row without `req_id` -> fetch the batch report and map by req_id.
   */
  private async resolveRows(
    rows: Array<Record<string, unknown>>,
    byKey: Map<string, { scanId: string; reqId: number; index: number; prompt: string }>,
    resolved: Map<string, RuntimeScanResult>,
  ): Promise<void> {
    const fallbackReports = new Map<string, string>(); // scanId -> reportId
    for (const r of rows) {
      const scanId = (r.scan_id as string) ?? '';
      const status = ((r.status as string) ?? '').toLowerCase();
      const terminal = status === 'complete' || status === 'completed' || status === 'failed';
      if (!terminal) continue;

      if (r.req_id != null) {
        const key = entryKey(scanId, r.req_id as number);
        const entry = byKey.get(key);
        if (!entry) continue;
        if (status === 'failed') {
          resolved.set(key, failedResult(entry.prompt, scanId));
        } else {
          resolved.set(
            key,
            scanResponseToResult((r.result as Record<string, unknown>) ?? {}, entry.prompt),
          );
        }
      } else {
        // batch-level terminal row without per-req_id breakdown -> queue report fallback
        const reportId =
          ((r.result as Record<string, unknown> | undefined)?.report_id as string) ??
          (r.report_id as string) ??
          '';
        if (reportId) fallbackReports.set(scanId, reportId);
        else {
          // no way to recover detail: fail every entry under this scan_id
          for (const [key, entry] of byKey) {
            if (entry.scanId === scanId && !resolved.has(key)) {
              resolved.set(key, failedResult(entry.prompt, scanId));
            }
          }
        }
      }
    }

    for (const [scanId, reportId] of fallbackReports) {
      const reports = (await this.scanner.queryByReportIds([reportId])) as Array<
        Record<string, unknown>
      >;
      for (const report of reports) {
        const reqId = report.req_id as number | undefined;
        if (reqId == null) continue;
        const key = entryKey(scanId, reqId);
        const entry = byKey.get(key);
        if (entry && !resolved.has(key)) {
          resolved.set(key, threatReportToResult(report, entry.prompt));
        }
      }
    }
  }
```

Add this module-level helper next to the other helpers (after `threatReportToResult`):

```ts
/** A RuntimeScanResult representing a failed async scan (safe-default allow). */
function failedResult(prompt: string, scanId: string): RuntimeScanResult {
  return {
    prompt,
    response: undefined,
    scanId,
    reportId: '',
    action: 'allow',
    category: 'error',
    triggered: false,
    detections: {},
  };
}
```

- [ ] **Step 6: Run tests**

Run: `pnpm test -- tests/unit/airs/runtime.spec.ts -t "submitBatch"`
Run: `pnpm test -- tests/unit/airs/runtime.spec.ts -t "pollBatch"`
Expected: PASS for all cases (per-req_id, report fallback, failed entry).

- [ ] **Step 7: Commit**

```bash
git add src/airs/runtime.ts tests/unit/airs/runtime.spec.ts
git commit -m "feat(runtime): submitBatch + pollBatch with per-prompt result mapping"
```

---

## Task 4: `getScanResults` / `getScanReports` + extended CSV

**Files:**
- Modify: `src/airs/runtime.ts` — add retrieval wrappers, extend CSV helpers
- Test: `tests/unit/airs/runtime.spec.ts`

- [ ] **Step 1: Write failing tests**

Add to `tests/unit/airs/runtime.spec.ts`:

```ts
describe('getScanResults / getScanReports', () => {
  it('chunks scan-ids into calls of 5 and concatenates results', async () => {
    mockScannerInstance.queryByScanIds
      .mockResolvedValueOnce([{ scan_id: 'a' }, { scan_id: 'b' }, { scan_id: 'c' }, { scan_id: 'd' }, { scan_id: 'e' }])
      .mockResolvedValueOnce([{ scan_id: 'f' }]);
    const out = await service.getScanResults(['a', 'b', 'c', 'd', 'e', 'f']);
    expect(mockScannerInstance.queryByScanIds).toHaveBeenCalledTimes(2);
    expect(out.map((r) => r.scan_id)).toEqual(['a', 'b', 'c', 'd', 'e', 'f']);
  });

  it('getScanReports delegates to queryByReportIds', async () => {
    mockScannerInstance.queryByReportIds.mockResolvedValueOnce([{ report_id: 'R1' }]);
    const out = await service.getScanReports(['R1']);
    expect(mockScannerInstance.queryByReportIds).toHaveBeenCalledWith(['R1']);
    expect(out).toEqual([{ report_id: 'R1' }]);
  });
});

describe('formatResultsCsv', () => {
  it('emits header with detection columns and one row per result', () => {
    const csv = SdkRuntimeService.formatResultsCsv([
      {
        prompt: 'say "hi"',
        scanId: 's1',
        reportId: 'R1',
        action: 'block',
        category: 'malicious',
        triggered: true,
        detections: { injection: true, dlp: false },
      },
    ]);
    const [header, row] = csv.split('\n');
    expect(header).toBe(
      'prompt,action,category,triggered,topic_violation,injection,toxic_content,dlp,url_cats,malicious_code,scan_id,report_id',
    );
    // injection true, dlp false, others default false; quotes doubled
    expect(row).toBe(
      '"say ""hi""","block","malicious","true","false","true","false","false","false","false","s1","R1"',
    );
  });
});
```

- [ ] **Step 2: Run to verify failure**

Run: `pnpm test -- tests/unit/airs/runtime.spec.ts -t "getScanResults"`
Run: `pnpm test -- tests/unit/airs/runtime.spec.ts -t "formatResultsCsv"`
Expected: FAIL — `getScanResults`/`getScanReports` missing; CSV header lacks detection columns.

- [ ] **Step 3: Add retrieval wrappers**

In `src/airs/runtime.ts`, inside the class (after `pollBatch`/`resolveRows`), add:

```ts
  async getScanResults(scanIds: string[]): Promise<ScanIdResult[]> {
    const out: ScanIdResult[] = [];
    for (let i = 0; i < scanIds.length; i += SDK_BATCH_MAX) {
      const chunk = scanIds.slice(i, i + SDK_BATCH_MAX);
      const rows = await this.scanner.queryByScanIds(chunk);
      out.push(...(rows as ScanIdResult[]));
    }
    return out;
  }

  async getScanReports(reportIds: string[]): Promise<ThreatScanReport[]> {
    const out: ThreatScanReport[] = [];
    for (let i = 0; i < reportIds.length; i += SDK_BATCH_MAX) {
      const chunk = reportIds.slice(i, i + SDK_BATCH_MAX);
      const rows = await this.scanner.queryByReportIds(chunk);
      out.push(...(rows as ThreatScanReport[]));
    }
    return out;
  }
```

- [ ] **Step 4: Rewrite CSV helpers**

Replace the static `formatResultsCsv` (currently lines ~198-205) with three statics:

```ts
  /** CSV header including per-detector columns. */
  static csvHeader(): string {
    return ['prompt', 'action', 'category', 'triggered', ...DETECTION_KEYS, 'scan_id', 'report_id'].join(
      ',',
    );
  }

  /** One CSV row for a result; every field double-quoted, inner quotes doubled. */
  static csvRow(r: RuntimeScanResult): string {
    const cells = [
      r.prompt,
      r.action,
      r.category,
      String(r.triggered),
      ...DETECTION_KEYS.map((k) => String(!!r.detections[k])),
      r.scanId,
      r.reportId,
    ];
    return cells.map((c) => `"${String(c).replace(/"/g, '""')}"`).join(',');
  }

  static formatResultsCsv(results: RuntimeScanResult[]): string {
    return [SdkRuntimeService.csvHeader(), ...results.map((r) => SdkRuntimeService.csvRow(r))].join(
      '\n',
    );
  }
```

- [ ] **Step 5: Run tests + full runtime suite + type-check**

Run: `pnpm test -- tests/unit/airs/runtime.spec.ts`
Expected: PASS (all groups).
Run: `pnpm tsc --noEmit`
Expected: errors ONLY in `src/cli/commands/runtime.ts` (old method calls) — fixed in Task 6.

- [ ] **Step 6: Commit**

```bash
git add src/airs/runtime.ts tests/unit/airs/runtime.spec.ts
git commit -m "feat(runtime): scan/report retrieval wrappers + detection CSV columns"
```

---

## Task 5: v2 bulk-scan state with legacy rejection

**Files:**
- Modify: `src/cli/bulk-scan-state.ts`
- Test (new): `tests/unit/cli/bulk-scan-state.spec.ts`

- [ ] **Step 1: Write failing tests**

Create `tests/unit/cli/bulk-scan-state.spec.ts`:

```ts
import * as fs from 'node:fs/promises';
import * as os from 'node:os';
import * as path from 'node:path';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import {
  type BulkScanState,
  loadBulkScanState,
  saveBulkScanState,
} from '../../../src/cli/bulk-scan-state.js';

describe('bulk-scan-state', () => {
  let dir: string;
  beforeEach(async () => {
    dir = await fs.mkdtemp(path.join(os.tmpdir(), 'bss-'));
  });
  afterEach(async () => {
    await fs.rm(dir, { recursive: true, force: true });
  });

  const v2: BulkScanState = {
    version: 2,
    profile: 'prof',
    sessionId: 'sess',
    outputFile: 'out.csv',
    batchSize: 25,
    batches: [
      {
        scanIds: ['s1'],
        entries: [{ scanId: 's1', reqId: 0, index: 0, prompt: 'hi' }],
        done: false,
      },
    ],
  };

  it('round-trips a v2 state file', async () => {
    const p = await saveBulkScanState(v2, dir);
    const loaded = await loadBulkScanState(p);
    expect(loaded.version).toBe(2);
    expect(loaded.profile).toBe('prof');
    expect(loaded.batches[0].entries[0].prompt).toBe('hi');
    expect(loaded.timestamp).toBeTypeOf('string');
  });

  it('rejects a legacy v1 state file with a clear error', async () => {
    const legacy = path.join(dir, 'legacy.bulk-scan.json');
    await fs.writeFile(
      legacy,
      JSON.stringify({ scanIds: ['a', 'b'], profile: 'p', promptCount: 10 }),
      'utf-8',
    );
    await expect(loadBulkScanState(legacy)).rejects.toThrow(/legacy|re-run bulk-scan/i);
  });
});
```

- [ ] **Step 2: Run to verify failure**

Run: `pnpm test -- tests/unit/cli/bulk-scan-state.spec.ts`
Expected: FAIL — new `BulkScanState` shape/exports and legacy guard don't exist.

- [ ] **Step 3: Rewrite `bulk-scan-state.ts`**

Replace the entire file with:

```ts
import * as fs from 'node:fs/promises';
import * as path from 'node:path';

/** One submitted batch persisted for crash-safe resume. */
export interface BulkScanBatchState {
  scanIds: string[];
  entries: { scanId: string; reqId: number; index: number; prompt: string }[];
  done: boolean;
}

/** Resumable bulk-scan state (schema v2 — persists prompts for full CSV on resume). */
export interface BulkScanState {
  version: 2;
  profile: string;
  sessionId?: string;
  outputFile: string;
  batchSize: number;
  timestamp?: string;
  batches: BulkScanBatchState[];
}

/**
 * Persist bulk scan state to a JSON file so polling can resume after a crash
 * or rate-limit. The same file is rewritten as batches complete when `filePath`
 * is supplied.
 */
export async function saveBulkScanState(
  state: BulkScanState,
  dir: string,
  filePath?: string,
): Promise<string> {
  await fs.mkdir(dir, { recursive: true });
  const target =
    filePath ?? path.join(dir, `${new Date().toISOString().replace(/[:.]/g, '-')}.bulk-scan.json`);
  const payload: BulkScanState = { ...state, timestamp: new Date().toISOString() };
  await fs.writeFile(target, JSON.stringify(payload, null, 2), 'utf-8');
  return target;
}

/** Load a v2 bulk-scan state file. Legacy v1 files are rejected with a clear error. */
export async function loadBulkScanState(filePath: string): Promise<BulkScanState> {
  const raw = await fs.readFile(filePath, 'utf-8');
  const parsed = JSON.parse(raw) as Partial<BulkScanState> & { promptCount?: number };
  if (parsed.version !== 2 || !Array.isArray(parsed.batches)) {
    throw new Error(
      'This bulk-scan state file predates prompt persistence and cannot be resumed. Re-run bulk-scan.',
    );
  }
  return parsed as BulkScanState;
}
```

- [ ] **Step 4: Run tests**

Run: `pnpm test -- tests/unit/cli/bulk-scan-state.spec.ts`
Expected: PASS (round-trip + legacy rejection).

- [ ] **Step 5: Commit**

```bash
git add src/cli/bulk-scan-state.ts tests/unit/cli/bulk-scan-state.spec.ts
git commit -m "feat(bulk-scan): v2 resumable state with prompt persistence, reject legacy v1"
```

---

## Task 6: Rewrite `bulk-scan` + `resume-poll` CLI commands

**Files:**
- Modify: `src/cli/commands/runtime.ts` — `bulk-scan` action (lines ~104-170), `resume-poll` action (lines ~216-266)

- [ ] **Step 1: Add an incremental-CSV helper near the top of the file**

In `src/cli/commands/runtime.ts`, after the `renderScanResult` function (ends line 66), add:

```ts
/** Append result rows to a CSV file, writing the header first if the file is new/empty. */
async function appendCsvRows(outputPath: string, results: RuntimeScanResult[]): Promise<void> {
  let needsHeader = true;
  try {
    const stat = await import('node:fs/promises').then((m) => m.stat(outputPath));
    needsHeader = stat.size === 0;
  } catch {
    needsHeader = true; // file does not exist yet
  }
  const lines = results.map((r) => SdkRuntimeService.csvRow(r));
  const chunk = (needsHeader ? [SdkRuntimeService.csvHeader(), ...lines] : lines).join('\n') + '\n';
  await import('node:fs/promises').then((m) => m.appendFile(outputPath, chunk, 'utf-8'));
}
```

(The file already imports `writeFile`/`readFile` from `node:fs/promises` at line 2; you may instead add `appendFile, stat` to that existing import and call them directly. Either is fine — keep it consistent with the file's style.)

- [ ] **Step 2: Rewrite the `bulk-scan` action body**

Replace the `bulkScan.action(async (opts) => { ... })` body (lines ~104-170). Keep the `.requiredOption`/`.option` chain but add `--batch-size`:

Add to the option chain (after `--session-id`, before `.addHelpText`):

```ts
    .option('--batch-size <n>', 'Prompts per sequential batch', String(DEFAULT_BATCH_SIZE))
```

Replace the action body with:

```ts
  bulkScan.action(async (opts) => {
    resolveDeprecatedAliases(bulkScan, opts);
    if (!opts.file) {
      usageError('--file <file> is required');
    }
    try {
      const config = await loadConfig({});
      if (!config.airsApiKey && !config.airsApiToken) {
        fail(new Error('PANW_AI_SEC_API_KEY or PANW_AI_SEC_API_TOKEN is required'));
      }

      const raw = await readFile(opts.file, 'utf-8');
      const prompts = parseInputFile(raw, opts.file);
      if (prompts.length === 0) {
        usageError('No prompts found in input file');
      }

      const batchSize = Math.max(1, Number.parseInt(opts.batchSize, 10) || DEFAULT_BATCH_SIZE);
      const sessionId = opts.sessionId ?? `prisma-airs-cli-bulk-${Date.now().toString(36)}`;
      const outputPath = opts.outputFile ?? `${opts.profile.replace(/\s+/g, '-')}-bulk-scan.csv`;
      const stateDir = config.dataDir.replace(/\/runs$/, '/bulk-scans');

      const service = new SdkRuntimeService(runtimeInitOptions(config));
      ui.status('Prisma AIRS Bulk Scan');
      ui.status(`Profile:  ${opts.profile}`);
      ui.status(`Session:  ${sessionId}`);
      ui.status(`Prompts:  ${prompts.length}`);
      ui.status(`Batches:  ${Math.ceil(prompts.length / batchSize)} (size ${batchSize})`);

      // Fresh output file so appends start with a header.
      await writeFile(outputPath, '', 'utf-8');

      const state: BulkScanState = {
        version: 2,
        profile: opts.profile,
        sessionId,
        outputFile: outputPath,
        batchSize,
        batches: [],
      };
      let statePath: string | undefined;
      let blocked = 0;
      let allowed = 0;

      for (let start = 0; start < prompts.length; start += batchSize) {
        const slice = prompts
          .slice(start, start + batchSize)
          .map((prompt, i) => ({ index: start + i, prompt }));
        ui.status(`Submitting batch ${Math.floor(start / batchSize) + 1}...`);
        const batch = await service.submitBatch(opts.profile, slice, sessionId);
        state.batches.push(batch);
        statePath = await saveBulkScanState(state, stateDir, statePath);

        const results = await service.pollBatch(batch, undefined, {
          onRetry: (attempt, delayMs) => {
            ui.status(`⚠ Rate limited — retry ${attempt} in ${(delayMs / 1000).toFixed(0)}s...`);
          },
        });
        await appendCsvRows(outputPath, results);
        batch.done = true;
        statePath = await saveBulkScanState(state, stateDir, statePath);

        blocked += results.filter((r) => r.action === 'block').length;
        allowed += results.filter((r) => r.action === 'allow').length;
        ui.status(`Batch complete — ${results.length} scanned (${blocked} blocked so far)`);
      }

      ui.header('Bulk Scan Complete');
      ui.keyValue([
        ['Total', prompts.length],
        ['Blocked', chalk.red(String(blocked))],
        ['Allowed', chalk.green(String(allowed))],
        ['Output', chalk.cyan(outputPath)],
        ['State', chalk.dim(statePath ?? '')],
      ]);
    } catch (err) {
      fail(err);
    }
  });
```

- [ ] **Step 3: Update imports for the new state type**

The command file imports `loadBulkScanState, saveBulkScanState` at line 15. Change to also import the type:

```ts
import { type BulkScanState, loadBulkScanState, saveBulkScanState } from '../bulk-scan-state.js';
```

- [ ] **Step 4: Rewrite the `resume-poll` action body**

Replace the `resumePoll.action(...)` body (lines ~226-266) with:

```ts
  resumePoll.action(async (stateFile: string, opts) => {
    resolveDeprecatedAliases(resumePoll, opts);
    try {
      const config = await loadConfig({});
      if (!config.airsApiKey && !config.airsApiToken) {
        fail(new Error('PANW_AI_SEC_API_KEY or PANW_AI_SEC_API_TOKEN is required'));
      }

      const state = await loadBulkScanState(stateFile);
      const service = new SdkRuntimeService(runtimeInitOptions(config));
      const outputPath = opts.outputFile ?? state.outputFile;
      const pending = state.batches.filter((b) => !b.done);

      ui.status('Prisma AIRS Resume Poll');
      ui.status(`Profile:  ${state.profile}`);
      ui.status(`Batches:  ${pending.length} pending / ${state.batches.length} total`);

      // Only (re)write header if the output file is missing/empty.
      let blocked = 0;
      let allowed = 0;
      for (const batch of pending) {
        const results = await service.pollBatch(batch, undefined, {
          onRetry: (attempt, delayMs) => {
            ui.status(`⚠ Rate limited — retry ${attempt} in ${(delayMs / 1000).toFixed(0)}s...`);
          },
        });
        await appendCsvRows(outputPath, results);
        batch.done = true;
        await saveBulkScanState(state, config.dataDir.replace(/\/runs$/, '/bulk-scans'), stateFile);
        blocked += results.filter((r) => r.action === 'block').length;
        allowed += results.filter((r) => r.action === 'allow').length;
      }

      ui.header('Resume Poll Complete');
      ui.keyValue([
        ['Resumed batches', pending.length],
        ['Blocked', chalk.red(String(blocked))],
        ['Allowed', chalk.green(String(allowed))],
        ['Output', chalk.cyan(outputPath)],
      ]);
    } catch (err) {
      fail(err);
    }
  });
```

- [ ] **Step 5: Type-check + build**

Run: `pnpm tsc --noEmit`
Expected: PASS (no errors). If `RuntimeScanResult` is unused-imported anywhere, Biome/tsc will flag it — it IS still used by `renderScanResult` and `appendCsvRows`.

Run: `pnpm run build`
Expected: tsup build succeeds.

- [ ] **Step 6: Smoke-test the CLI wiring (no creds needed for --help)**

Run: `pnpm run dev -- runtime bulk-scan --help`
Expected: help shows `--batch-size <n>` with default 25.

- [ ] **Step 7: Commit**

```bash
git add src/cli/commands/runtime.ts
git commit -m "feat(bulk-scan): sequential batches, --batch-size, incremental CSV, v2 resume"
```

---

## Task 7: `get-scan` command + renderers

**Files:**
- Modify: `src/cli/renderer/runtime.ts` — add `renderScanIdResults`, `renderThreatReports`
- Modify: `src/cli/commands/runtime.ts` — register `get-scan`; import the renderers

- [ ] **Step 1: Add the renderers**

In `src/cli/renderer/runtime.ts`, confirm the top imports include yaml (add if missing) — mirror `redteam.ts`:

```ts
import { dump as yamlDump } from 'js-yaml';
```

Then append these two functions:

```ts
/** Render scan-id query results (verdict/action + detection flags). */
export function renderScanIdResults(
  results: Array<Record<string, unknown>>,
  format: OutputFormat = 'pretty',
): void {
  if (format === 'json') {
    console.log(JSON.stringify(results, null, 2));
    return;
  }
  if (format === 'yaml') {
    console.log(yamlDump(results));
    return;
  }
  if (results.length === 0) {
    ui.dim('No scan results.');
    return;
  }
  for (const r of results) {
    const res = (r.result as Record<string, unknown>) ?? {};
    const action = (res.action as string) ?? 'unknown';
    ui.header('Scan Result');
    ui.keyValue([
      ['Scan ID', chalk.dim((r.scan_id as string) ?? '')],
      ['Report ID', chalk.dim((res.report_id as string) ?? '')],
      ['Status', (r.status as string) ?? 'unknown'],
      ['Category', (res.category as string) ?? 'unknown'],
      ['Action', action === 'block' ? chalk.red('BLOCK') : chalk.green(action.toUpperCase())],
    ]);
    const detected = (res.prompt_detected as Record<string, boolean> | undefined) ?? {};
    const fired = Object.entries(detected).filter(([, v]) => v);
    if (fired.length > 0) {
      ui.section('Detections');
      for (const [k] of fired) ui.bullet(k, 'flag');
    }
  }
}

/** Render detailed threat reports (per-detector verdict/action). */
export function renderThreatReports(
  reports: Array<Record<string, unknown>>,
  format: OutputFormat = 'pretty',
): void {
  if (format === 'json') {
    console.log(JSON.stringify(reports, null, 2));
    return;
  }
  if (format === 'yaml') {
    console.log(yamlDump(reports));
    return;
  }
  if (reports.length === 0) {
    ui.dim('No threat reports.');
    return;
  }
  for (const report of reports) {
    ui.header('Threat Report');
    ui.keyValue([
      ['Report ID', chalk.dim((report.report_id as string) ?? '')],
      ['Scan ID', chalk.dim((report.scan_id as string) ?? '')],
      ['Req ID', String(report.req_id ?? '')],
    ]);
    const drs = (report.detection_results as Array<Record<string, unknown>> | undefined) ?? [];
    if (drs.length > 0) {
      ui.table(
        [
          { key: 'service', label: 'Detector' },
          { key: 'verdict', label: 'Verdict' },
          { key: 'action', label: 'Action' },
        ],
        drs.map((d) => ({
          service: (d.detection_service as string) ?? '',
          verdict: (d.verdict as string) ?? '',
          action: (d.action as string) ?? '',
        })),
      );
    }
  }
}
```

- [ ] **Step 2: Register the `get-scan` command**

In `src/cli/commands/runtime.ts`, add the renderers to the renderer import block (lines 21-39): add `renderScanIdResults,` and `renderThreatReports,`.

Then, right after the `scan` command registration (the `.command('scan <prompt>')` block, which ends around line 320 — place this immediately after its `.action(...)`), add:

```ts
  // -----------------------------------------------------------------------
  // runtime get-scan — retrieve scan results / threat reports by ID
  // -----------------------------------------------------------------------
  runtime
    .command('get-scan <ids...>')
    .description('Fetch scan results (scan-id) or threat reports (report-id) by ID')
    .option('--output <format>', 'Output format: pretty, json, yaml', 'pretty')
    .addHelpText(
      'after',
      examples(
        'airs runtime get-scan 550e8400-e29b-41d4-a716-446655440000',
        'airs runtime get-scan R0000000000000000001 --output json',
      ),
    )
    .action(async (ids: string[], opts) => {
      try {
        const fmt = opts.output as OutputFormat;
        const uuidRe = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;
        const scanIds: string[] = [];
        const reportIds: string[] = [];
        for (const id of ids) {
          if (/^r/i.test(id)) reportIds.push(id);
          else if (uuidRe.test(id)) scanIds.push(id);
          else usageError(`Unrecognized ID (expected scan UUID or R-prefixed report ID): ${id}`);
        }

        const config = await loadConfig({});
        if (!config.airsApiKey && !config.airsApiToken) {
          fail(new Error('PANW_AI_SEC_API_KEY or PANW_AI_SEC_API_TOKEN is required'));
        }
        const service = new SdkRuntimeService(runtimeInitOptions(config));

        if (scanIds.length > 0) {
          const results = await service.getScanResults(scanIds);
          renderScanIdResults(results as Array<Record<string, unknown>>, fmt);
        }
        if (reportIds.length > 0) {
          const reports = await service.getScanReports(reportIds);
          renderThreatReports(reports as Array<Record<string, unknown>>, fmt);
        }
      } catch (err) {
        fail(err);
      }
    });
```

- [ ] **Step 3: Type-check + build**

Run: `pnpm tsc --noEmit`
Expected: PASS.
Run: `pnpm run build`
Expected: succeeds.

- [ ] **Step 4: Smoke-test help + ID classification**

Run: `pnpm run dev -- runtime get-scan --help`
Expected: shows the two examples and `--output`.
Run: `pnpm run dev -- runtime get-scan not-a-valid-id`
Expected: exits 2 with "Unrecognized ID ...".

- [ ] **Step 5: Commit**

```bash
git add src/cli/renderer/runtime.ts src/cli/commands/runtime.ts
git commit -m "feat(runtime): add get-scan command + scan-result/threat-report renderers"
```

---

## Task 8: Quality gates, docs, changeset

**Files:**
- Modify: `CLAUDE.md` (Runtime Scanning section), `docs-site/docs/about/release-notes.md`
- Create: `.changeset/bulk-scan-fix.md`

- [ ] **Step 1: Full test suite + coverage-sensitive check**

Run: `pnpm test`
Expected: all pass.
Run: `pnpm run lint`
Expected: no errors (fix any with `pnpm run lint:fix`).
Run: `pnpm tsc --noEmit`
Expected: no errors.

- [ ] **Step 2: Update `CLAUDE.md` Runtime Scanning bullets**

In `CLAUDE.md`, under "### Runtime Scanning", replace the `submitBulkScan`/`pollResults`/`formatResultsCsv` bullets and the bulk-scan CLI lines with:

```md
- `submitBatch()` — submits one batch of indexed prompts (chunked into ≤5-object `asyncScan` calls); returns a `SubmittedBatch` mapping each prompt to `(scan_id, req_id)`.
- `pollBatch()` — polls a batch's scan IDs (≤5/query) until every prompt completes; maps `ScanIdResult` rows back per-prompt by `(scan_id, req_id)` with full detection data, falling back to `queryByReportIds` when rows lack `req_id`. Rate-limit backoff preserved.
- `getScanResults()` / `getScanReports()` — retrieve scan results / threat reports by ID (chunked ≤5). Power `runtime get-scan`.
- `formatResultsCsv()` / `csvHeader()` / `csvRow()` — CSV with per-detector columns (topic_violation, injection, toxic_content, dlp, url_cats, malicious_code).
- CLI: `airs runtime bulk-scan --profile <name> --file <file> [--output-file <file>] [--session-id <id>] [--batch-size <n>]` — sequential batches (default 25), incremental CSV, crash-safe v2 state.
- CLI: `airs runtime resume-poll <stateFile> [--output-file <file>]` — resumes not-done batches; prompts restored from state. Legacy v1 state files are rejected with a clear error.
- CLI: `airs runtime get-scan <id...>` — fetch scan result (scan UUID) or threat report (R-prefixed) by ID; `--output pretty|json|yaml`.
```

- [ ] **Step 3: Add release notes**

In `docs-site/docs/about/release-notes.md`, add an entry at the top for the next version (check current `package.json` version and use the next patch/minor — this is a bugfix + feature, so **minor**):

```md
## Unreleased

### Fixed
- `runtime bulk-scan` now correctly maps every prompt to its scan result. Previously batches of >5 prompts dropped 4 of every 5 prompts and misaligned output rows.
- Bulk-scan CSV now includes real detection data (`triggered` + per-detector columns) instead of always-empty/false values.
- `runtime resume-poll` now restores prompt text in resumed output (prompts are persisted in the v2 state file).

### Added
- `runtime bulk-scan --batch-size <n>` — process prompts in sequential batches (default 25) with incremental, crash-safe CSV output.
- `runtime get-scan <id...>` — retrieve scan results (scan UUID) or detailed threat reports (R-prefixed report ID) by ID, with `--output pretty|json|yaml`.
```

- [ ] **Step 4: Add a changeset**

Create `.changeset/bulk-scan-fix.md` (check the package name in `package.json` `name` field and use it verbatim):

```md
---
"@cdot65/prisma-airs-cli": minor
---

Fix broken `runtime bulk-scan` (correct per-prompt result mapping, real detection data, incremental crash-safe CSV output, `--batch-size`, resumable state) and add `runtime get-scan <id...>` to retrieve scan results and threat reports by ID.
```

Note: verify the exact package name — if `package.json` `"name"` differs from `@cdot65/prisma-airs-cli`, use the actual value.

- [ ] **Step 5: Final full gate**

Run: `pnpm test && pnpm run lint && pnpm tsc --noEmit && pnpm run build`
Expected: all green.

- [ ] **Step 6: Commit**

```bash
git add CLAUDE.md docs-site/docs/about/release-notes.md .changeset/bulk-scan-fix.md
git commit -m "docs: bulk-scan rewrite + get-scan notes, changeset"
```

---

## Notes for the implementer

- **ESM import specifiers use `.js`** even for `.ts` files — match the existing style.
- **Never scan/reference profiles by ID** — bulk-scan uses `profile_name` (already correct).
- **Verify the live async shape early (optional but recommended):** the per-`req_id` mapping in `pollBatch` is the primary path; the `queryByReportIds` fallback covers the batch-level-only case. Both are unit-tested with mocks, so implementation does not block on live verification — but if you have creds, run one real `bulk-scan` with >5 prompts and confirm the CSV rows align 1:1 with input prompts.
- **Coverage:** `src/cli/**` is excluded from coverage; the service-layer tests in Tasks 2–5 carry the coverage weight (targets: lines 90 / functions 95 / branches 80).
- **Do not hand-edit `package.json` version** — the changeset drives it.
