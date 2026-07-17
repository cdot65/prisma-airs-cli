import {
  Content,
  type InitOptions,
  init,
  MAX_NUMBER_OF_BATCH_SCAN_OBJECTS,
  type ScanCallOptions,
  Scanner,
} from '@cdot65/prisma-airs-sdk';
import type {
  BulkScanResult,
  IndexedPrompt,
  ReliableRuntimeService,
  RuntimeScanResult,
  SubmittedBatch,
} from './types.js';

/** Maximum async request objects accepted by the installed AIRS SDK. */
export const SDK_ASYNC_BATCH_SIZE = MAX_NUMBER_OF_BATCH_SCAN_OBJECTS;
const DEFAULT_POLL_INTERVAL_MS = 5000;
const DEFAULT_MAX_RETRIES = 5;
const DEFAULT_BASE_DELAY_MS = 10_000;
const DEFAULT_MAX_NO_PROGRESS_POLLS = 120;
const NO_SDK_RETRIES: ScanCallOptions = { numRetries: 0 };

/** Prompt-side detector flags returned by the AIRS runtime API. */
export const RUNTIME_DETECTION_KEYS = [
  'topic_violation',
  'injection',
  'toxic_content',
  'dlp',
  'url_cats',
  'malicious_code',
  'source_code',
  'agent',
] as const;

const REPORT_DETECTION_KEYS: Record<string, (typeof RUNTIME_DETECTION_KEYS)[number]> = {
  topic_guardrails: 'topic_violation',
  topic_violation: 'topic_violation',
  pi: 'injection',
  prompt_injection: 'injection',
  injection: 'injection',
  tc: 'toxic_content',
  toxic_content: 'toxic_content',
  dlp: 'dlp',
  uf: 'url_cats',
  url_filtering: 'url_cats',
  url_cats: 'url_cats',
  mc: 'malicious_code',
  malicious_code: 'malicious_code',
  source_code: 'source_code',
  agent: 'agent',
};

export interface PollRetryOptions {
  /** Max retries per rate-limit error before giving up. Default: 5. */
  maxRetries?: number;
  /** Base delay in ms for exponential backoff. Default: 10000. */
  baseDelayMs?: number;
  /** Stop after this many consecutive successful polls resolve no new prompts. Default: 120. */
  maxNoProgressPolls?: number;
  /** Called on each retry with (attempt, delayMs). */
  onRetry?: (attempt: number, delayMs: number) => void;
  /** Called whenever newly terminal prompt results are resolved. */
  onProgress?: (results: BulkScanResult[]) => void | Promise<void>;
}

function isRateLimitError(err: unknown): boolean {
  if ((err as { statusCode?: number } | undefined)?.statusCode === 429) return true;
  if (err instanceof Error) {
    const msg = err.message.toLowerCase();
    return msg.includes('rate limit') || msg.includes('rate_limit') || msg.includes('429');
  }
  return false;
}

function isDefiniteRateLimitError(err: unknown): boolean {
  const metadata = err as { failureKind?: string; statusCode?: number } | undefined;
  return metadata?.failureKind === 'http' && metadata.statusCode === 429;
}

function runtimeDetections(value: unknown): Record<string, boolean> {
  const source = (value as Record<string, unknown> | undefined) ?? {};
  return Object.fromEntries(
    RUNTIME_DETECTION_KEYS.filter((key) => typeof source[key] === 'boolean').map((key) => [
      key,
      source[key] as boolean,
    ]),
  );
}

type NormalizedRuntimeResult = Omit<BulkScanResult, 'index' | 'reqId'>;

function runtimeAction(value: unknown): BulkScanResult['action'] {
  const normalized = typeof value === 'string' ? value.toLowerCase() : '';
  if (normalized === 'allow' || normalized === 'block') return normalized;
  return 'failed';
}

function scanResponseToResult(
  response: Record<string, unknown>,
  prompt: string,
): NormalizedRuntimeResult {
  const detections = runtimeDetections(response.prompt_detected);
  const action = runtimeAction(response.action);
  const failed = response.error === true || response.timeout === true || action === 'failed';
  const errors = Array.isArray(response.errors)
    ? response.errors.map((entry) => {
        const detail = entry as Record<string, unknown>;
        return [detail.feature, detail.status, detail.content_type].filter(Boolean).join(': ');
      })
    : [];
  return {
    prompt,
    response: undefined,
    scanId: (response.scan_id as string | undefined) ?? '',
    reportId: (response.report_id as string | undefined) ?? '',
    action: failed ? 'failed' : action,
    category: failed ? 'error' : ((response.category as string | undefined) ?? 'unknown'),
    triggered: RUNTIME_DETECTION_KEYS.some((key) => detections[key] === true),
    detections,
    ...(failed
      ? {
          error:
            errors.filter(Boolean).join('; ') ||
            (response.timeout === true
              ? 'AIRS scan timed out'
              : response.error === true
                ? 'AIRS scan failed'
                : `Unknown AIRS action: ${String(response.action ?? 'missing')}`),
        }
      : {}),
  };
}

function threatReportToResult(
  report: Record<string, unknown>,
  entry: SubmittedBatch['entries'][number],
): BulkScanResult {
  const detections: Record<string, boolean> = {};
  let sawBlock = false;
  let unexpectedAction: string | undefined;
  const detectionResults = Array.isArray(report.detection_results)
    ? (report.detection_results as Array<Record<string, unknown>>)
    : [];

  for (const detection of detectionResults) {
    const service = String(detection.detection_service ?? '').toLowerCase();
    const detectorAction = String(detection.action ?? '').toLowerCase();
    const verdict = String(detection.verdict ?? '').toLowerCase();
    const fired =
      detectorAction === 'block' ||
      ['malicious', 'unsafe', 'violation', 'detected'].includes(verdict);
    const namedKey = REPORT_DETECTION_KEYS[service] ?? service;
    const key = namedKey || (fired ? 'unknown' : '');
    if (key) detections[key] = detections[key] === true || fired;
    if (detectorAction === 'block') sawBlock = true;
    else if (detectorAction && detectorAction !== 'allow') unexpectedAction = detectorAction;
  }

  const triggered = Object.values(detections).some(Boolean);
  const action: BulkScanResult['action'] = sawBlock
    ? 'block'
    : unexpectedAction
      ? 'failed'
      : 'allow';
  return {
    index: entry.index,
    reqId: entry.reqId,
    prompt: entry.prompt,
    response: undefined,
    scanId: entry.scanId,
    reportId: (report.report_id as string | undefined) ?? '',
    action,
    category: action === 'failed' ? 'error' : triggered ? 'malicious' : 'benign',
    triggered,
    detections,
    ...(action === 'failed'
      ? { error: `Unknown AIRS action in threat report: ${unexpectedAction}` }
      : {}),
  };
}

function failedBulkResult(
  entry: SubmittedBatch['entries'][number],
  error = 'AIRS async scan failed',
): BulkScanResult {
  return {
    index: entry.index,
    reqId: entry.reqId,
    prompt: entry.prompt,
    response: undefined,
    scanId: entry.scanId,
    reportId: '',
    action: 'failed',
    category: 'error',
    triggered: false,
    detections: {},
    error,
  };
}

export class SdkRuntimeService implements ReliableRuntimeService {
  private scanner: Scanner;

  constructor(opts: InitOptions) {
    init(opts);
    this.scanner = new Scanner();
  }

  async scanPrompt(
    profileName: string,
    prompt: string,
    response?: string,
  ): Promise<RuntimeScanResult> {
    const contentOpts: Record<string, string> = { prompt };
    if (response) contentOpts.response = response;
    const content = new Content(contentOpts);

    const res = await this.scanner.syncScan({ profile_name: profileName }, content, undefined);
    const normalized = scanResponseToResult(res as unknown as Record<string, unknown>, prompt);
    return {
      ...normalized,
      response,
      action: normalized.action === 'block' ? 'block' : 'allow',
    };
  }

  async submitBatch(
    profileName: string,
    prompts: IndexedPrompt[],
    sessionId?: string,
    retryOpts?: PollRetryOptions,
  ): Promise<SubmittedBatch> {
    if (prompts.length < 1 || prompts.length > SDK_ASYNC_BATCH_SIZE) {
      throw new Error(`submitBatch requires between 1 and ${SDK_ASYNC_BATCH_SIZE} prompts`);
    }
    const promptIndices = new Set<number>();
    for (const prompt of prompts) {
      if (
        !Number.isSafeInteger(prompt.index) ||
        prompt.index < 0 ||
        promptIndices.has(prompt.index)
      ) {
        throw new Error('submitBatch requires a unique nonnegative safe index for every prompt');
      }
      promptIndices.add(prompt.index);
    }

    const scanObjects = prompts.map(({ index, prompt }) => ({
      req_id: index,
      scan_req: {
        ai_profile: { profile_name: profileName },
        contents: [{ prompt }],
        ...(sessionId ? { session_id: sessionId } : {}),
      },
    }));
    const maxRetries = retryOpts?.maxRetries ?? DEFAULT_MAX_RETRIES;
    const baseDelay = retryOpts?.baseDelayMs ?? DEFAULT_BASE_DELAY_MS;
    let retryAttempt = 0;
    let receipt: Awaited<ReturnType<Scanner['asyncScan']>>;
    while (true) {
      try {
        receipt = await this.scanner.asyncScan(scanObjects, NO_SDK_RETRIES);
        break;
      } catch (error) {
        if (!isDefiniteRateLimitError(error) || retryAttempt >= maxRetries) throw error;
        retryAttempt++;
        const retryAfterMs = (error as { retryAfterMs?: number }).retryAfterMs;
        const delayMs =
          typeof retryAfterMs === 'number' && Number.isFinite(retryAfterMs) && retryAfterMs >= 0
            ? retryAfterMs
            : baseDelay * 2 ** (retryAttempt - 1);
        retryOpts?.onRetry?.(retryAttempt, delayMs);
        await new Promise((resolve) => setTimeout(resolve, delayMs));
      }
    }

    return {
      scanId: receipt.scan_id,
      reportId: receipt.report_id,
      entries: prompts.map(({ index, prompt }) => ({
        scanId: receipt.scan_id,
        reqId: index,
        index,
        prompt,
      })),
    };
  }

  async pollBatch(
    batch: SubmittedBatch,
    intervalMs = DEFAULT_POLL_INTERVAL_MS,
    retryOpts?: PollRetryOptions,
  ): Promise<BulkScanResult[]> {
    if (batch.entries.length < 1 || batch.entries.length > SDK_ASYNC_BATCH_SIZE) {
      throw new Error(`pollBatch requires between 1 and ${SDK_ASYNC_BATCH_SIZE} receipt entries`);
    }
    const receiptIds = new Set<number>();
    for (const entry of batch.entries) {
      if (entry.scanId !== batch.scanId) {
        throw new Error(`Receipt entry scan ID ${entry.scanId} does not match ${batch.scanId}`);
      }
      if (
        !Number.isSafeInteger(entry.reqId) ||
        entry.reqId < 0 ||
        entry.reqId !== entry.index ||
        receiptIds.has(entry.reqId)
      ) {
        throw new Error('pollBatch requires a unique request ID matching each prompt index');
      }
      receiptIds.add(entry.reqId);
    }
    const maxRetries = retryOpts?.maxRetries ?? DEFAULT_MAX_RETRIES;
    const baseDelay = retryOpts?.baseDelayMs ?? DEFAULT_BASE_DELAY_MS;
    const maxNoProgressPolls = retryOpts?.maxNoProgressPolls ?? DEFAULT_MAX_NO_PROGRESS_POLLS;
    const entries = new Map(batch.entries.map((entry) => [entry.reqId, entry]));
    const resolved = new Map<number, BulkScanResult>();
    let retryLevel = 0;
    let noProgressPolls = 0;

    while (resolved.size < batch.entries.length) {
      let rows: Array<Record<string, unknown>>;
      try {
        rows = (await this.scanner.queryByScanIds([batch.scanId], NO_SDK_RETRIES)) as Array<
          Record<string, unknown>
        >;
      } catch (err) {
        if (isRateLimitError(err) && retryLevel < maxRetries) {
          retryLevel++;
          const retryAfterMs = (err as { retryAfterMs?: number }).retryAfterMs;
          const delayMs =
            typeof retryAfterMs === 'number' && Number.isFinite(retryAfterMs) && retryAfterMs >= 0
              ? retryAfterMs
              : baseDelay * 2 ** (retryLevel - 1);
          retryOpts?.onRetry?.(retryLevel, delayMs);
          await new Promise((resolve) => setTimeout(resolve, delayMs));
          continue;
        }
        throw err;
      }

      const resolvedBeforePoll = resolved.size;
      const fallbackReports = new Map<string, string>();
      for (const row of rows) {
        const scanId = (row.scan_id as string | undefined) ?? batch.scanId;
        const reqId = row.req_id as number | undefined;
        const status = ((row.status as string | undefined) ?? '').toLowerCase();
        const terminal = status === 'failed' || status === 'complete' || status === 'completed';
        if (reqId === undefined && terminal && scanId === batch.scanId) {
          if (status === 'failed') {
            const newlyFailed: BulkScanResult[] = [];
            for (const entry of batch.entries) {
              if (!resolved.has(entry.reqId)) {
                const result = failedBulkResult(entry);
                resolved.set(entry.reqId, result);
                newlyFailed.push(result);
              }
            }
            if (newlyFailed.length > 0) await retryOpts?.onProgress?.(newlyFailed);
            continue;
          }
          const reportId =
            ((row.result as Record<string, unknown> | undefined)?.report_id as
              | string
              | undefined) ??
            (row.report_id as string | undefined) ??
            batch.reportId;
          if (!reportId) {
            throw new Error(
              `AIRS result correlation failed for scan ${scanId}: terminal row has no request or report ID`,
            );
          }
          fallbackReports.set(scanId, reportId);
          continue;
        }
        const entry =
          reqId === undefined || scanId !== batch.scanId ? undefined : entries.get(reqId);
        if (!entry || resolved.has(entry.reqId)) continue;

        if (status === 'failed') {
          const result = failedBulkResult(entry);
          resolved.set(entry.reqId, result);
          await retryOpts?.onProgress?.([result]);
          continue;
        }

        if ((status === 'complete' || status === 'completed') && row.result) {
          const nestedResult = row.result as Record<string, unknown>;
          const nestedScanId = nestedResult.scan_id as string | undefined;
          if (nestedScanId && nestedScanId !== scanId) {
            throw new Error(
              `AIRS result correlation mismatch: nested scan ID ${nestedScanId} does not match ${scanId}`,
            );
          }
          const result: BulkScanResult = {
            ...scanResponseToResult(nestedResult, entry.prompt),
            scanId,
            index: entry.index,
            reqId: entry.reqId,
          };
          resolved.set(entry.reqId, result);
          await retryOpts?.onProgress?.([result]);
        }
      }

      if (fallbackReports.size > 0) {
        let reports: Array<Record<string, unknown>>;
        try {
          reports = (await this.scanner.queryByReportIds(
            [...new Set(fallbackReports.values())],
            NO_SDK_RETRIES,
          )) as Array<Record<string, unknown>>;
        } catch (err) {
          if (isRateLimitError(err) && retryLevel < maxRetries) {
            retryLevel++;
            const retryAfterMs = (err as { retryAfterMs?: number }).retryAfterMs;
            const delayMs =
              typeof retryAfterMs === 'number' && Number.isFinite(retryAfterMs) && retryAfterMs >= 0
                ? retryAfterMs
                : baseDelay * 2 ** (retryLevel - 1);
            retryOpts?.onRetry?.(retryLevel, delayMs);
            await new Promise((resolve) => setTimeout(resolve, delayMs));
            continue;
          }
          throw err;
        }

        for (const report of reports) {
          const reqId = report.req_id as number | undefined;
          const reportScanId = (report.scan_id as string | undefined) ?? batch.scanId;
          const expectedReportId = fallbackReports.get(reportScanId);
          const reportId = report.report_id as string | undefined;
          if (reportScanId !== batch.scanId || (reportId && reportId !== expectedReportId)) {
            throw new Error(
              `AIRS report correlation mismatch for scan ${batch.scanId}, request ${String(reqId)}`,
            );
          }
          const entry = reqId === undefined ? undefined : entries.get(reqId);
          if (!entry || resolved.has(entry.reqId)) continue;
          const result = threatReportToResult(report, entry);
          resolved.set(entry.reqId, result);
          await retryOpts?.onProgress?.([result]);
        }
      }

      if (retryLevel > 0) retryLevel--;

      if (resolved.size === resolvedBeforePoll) {
        noProgressPolls++;
      } else {
        noProgressPolls = 0;
      }
      if (noProgressPolls >= maxNoProgressPolls) {
        throw new Error(
          `AIRS polling made no progress after ${noProgressPolls} polls for scan ${batch.scanId}`,
        );
      }

      if (resolved.size < batch.entries.length) {
        await new Promise((resolve) => setTimeout(resolve, intervalMs));
      }
    }

    return [...resolved.values()].sort((left, right) => left.index - right.index);
  }

  /** @deprecated Use submitBatch to preserve per-prompt request correlation. */
  async submitBulkScan(
    profileName: string,
    prompts: string[],
    sessionId?: string,
  ): Promise<string[]> {
    const scanIds: string[] = [];

    for (let i = 0; i < prompts.length; i += SDK_ASYNC_BATCH_SIZE) {
      const batch = prompts.slice(i, i + SDK_ASYNC_BATCH_SIZE);
      const scanObjects = batch.map((prompt, idx) => ({
        req_id: i + idx,
        scan_req: {
          ai_profile: { profile_name: profileName },
          contents: [{ prompt }],
          ...(sessionId ? { session_id: sessionId } : {}),
        },
      }));

      const res = await this.scanner.asyncScan(scanObjects, NO_SDK_RETRIES);
      scanIds.push(res.scan_id);
    }

    return scanIds;
  }

  /**
   * Compatibility poller for callers that retained only batch scan IDs.
   * Nested detection data is preserved, but prompt text and per-request fan-out
   * cannot be reconstructed from scan IDs alone.
   * @deprecated Use pollBatch to preserve `(scan_id, req_id)` correlation and prompt text.
   */
  async pollResults(
    scanIds: string[],
    intervalMs = DEFAULT_POLL_INTERVAL_MS,
    retryOpts?: PollRetryOptions,
  ): Promise<RuntimeScanResult[]> {
    const maxRetries = retryOpts?.maxRetries ?? DEFAULT_MAX_RETRIES;
    const baseDelay = retryOpts?.baseDelayMs ?? DEFAULT_BASE_DELAY_MS;
    const completed = new Map<string, RuntimeScanResult>();
    const pending = new Set(scanIds);
    let retryLevel = 0;

    while (pending.size > 0) {
      // Query all pending IDs in batches of 5 per sweep
      const pendingIds = [...pending];
      let sweepCompleted = true;

      for (let b = 0; b < pendingIds.length; b += 5) {
        const batch = pendingIds.slice(b, b + 5);

        let results: unknown[];
        try {
          results = await this.scanner.queryByScanIds(batch);
        } catch (err) {
          if (isRateLimitError(err) && retryLevel < maxRetries) {
            retryLevel++;
            const delayMs = baseDelay * 2 ** (retryLevel - 1);
            retryOpts?.onRetry?.(retryLevel, delayMs);
            await new Promise((resolve) => setTimeout(resolve, delayMs));
            sweepCompleted = false;
            break; // restart sweep from the beginning
          }
          throw err;
        }

        this.processQueryResults(results, completed, pending);

        // Small inter-batch delay to avoid hammering the API
        if (b + 5 < pendingIds.length) {
          const batchDelay = retryLevel > 0 ? baseDelay : Math.min(baseDelay, 1000);
          await new Promise((resolve) => setTimeout(resolve, batchDelay));
        }
      }

      // Only decay retry level after a full sweep with no rate limit errors
      if (sweepCompleted && retryLevel > 0) {
        retryLevel = Math.max(0, retryLevel - 1);
      }

      if (pending.size > 0) {
        const sweepDelay = retryLevel > 0 ? baseDelay * 2 ** retryLevel : intervalMs;
        await new Promise((resolve) => setTimeout(resolve, sweepDelay));
      }
    }

    return scanIds.map((id) => completed.get(id) as RuntimeScanResult);
  }

  private processQueryResults(
    results: unknown[],
    completed: Map<string, RuntimeScanResult>,
    pending: Set<string>,
  ): void {
    for (const r of results as Array<Record<string, unknown>>) {
      const id = (r.scan_id as string) ?? '';
      const status = ((r.status as string) ?? '').toLowerCase();

      if ((status === 'complete' || status === 'completed') && r.result) {
        const result = r.result as Record<string, unknown>;
        const nestedScanId = result.scan_id as string | undefined;
        if (nestedScanId && nestedScanId !== id) {
          throw new Error(
            `AIRS result correlation mismatch: nested scan ID ${nestedScanId} does not match ${id}`,
          );
        }
        completed.set(id, {
          ...scanResponseToResult(result, ''),
          scanId: id,
          action: result.action === 'block' ? 'block' : 'allow',
        });
        pending.delete(id);
      } else if (status === 'failed') {
        completed.set(id, {
          prompt: '', // not available from async API
          response: undefined, // not available from async API
          scanId: id,
          reportId: '',
          action: 'allow',
          category: 'error',
          triggered: false,
          detections: {},
          error: 'AIRS async scan failed',
        });
        pending.delete(id);
      }
    }
  }

  static formatResultsCsv(results: Array<RuntimeScanResult | BulkScanResult>): string {
    const header = [
      'prompt',
      'action',
      'category',
      'triggered',
      ...RUNTIME_DETECTION_KEYS,
      'scan_id',
      'report_id',
      'error',
    ].join(',');
    const rows = results.map((r) => {
      const fields = [
        r.prompt,
        r.action,
        r.category,
        String(r.triggered),
        ...RUNTIME_DETECTION_KEYS.map((key) => String(r.detections[key] === true)),
        r.scanId,
        r.reportId,
        r.error ?? '',
      ];
      return fields.map((field) => `"${field.replace(/"/g, '""')}"`).join(',');
    });
    return [header, ...rows].join('\n');
  }
}
