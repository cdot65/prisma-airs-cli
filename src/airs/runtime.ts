import { Content, init, Scanner } from '@cdot65/prisma-airs-sdk';
import type { RuntimeScanResult, RuntimeService } from './types.js';

const BATCH_SIZE = 5;
const DEFAULT_POLL_INTERVAL_MS = 5000;
const DEFAULT_MAX_RETRIES = 5;
const DEFAULT_BASE_DELAY_MS = 10_000;

export interface PollRetryOptions {
  /** Max retries per rate-limit error before giving up. Default: 5. */
  maxRetries?: number;
  /** Base delay in ms for exponential backoff. Default: 10000. */
  baseDelayMs?: number;
  /** Called on each retry with (attempt, delayMs). */
  onRetry?: (attempt: number, delayMs: number) => void;
}

function isRateLimitError(err: unknown): boolean {
  if (err instanceof Error) {
    const msg = err.message.toLowerCase();
    return msg.includes('rate limit') || msg.includes('rate_limit') || msg.includes('429');
  }
  return false;
}

export class SdkRuntimeService implements RuntimeService {
  private scanner: InstanceType<typeof Scanner>;

  constructor(apiKey: string) {
    init({ apiKey });
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

    const detected = (res.prompt_detected as Record<string, boolean> | undefined) ?? {};
    // Runtime scanning aggregates all 6 detection types — intentionally broader than
    // the guardrail loop's topic_violation-only signal. Runtime is a general-purpose
    // firewall check, not topic-specific evaluation.
    const triggered = !!(
      detected.topic_violation ||
      detected.injection ||
      detected.toxic_content ||
      detected.dlp ||
      detected.url_cats ||
      detected.malicious_code
    );

    return {
      prompt,
      response,
      scanId: res.scan_id ?? '',
      reportId: res.report_id ?? '',
      action: res.action === 'block' ? 'block' : 'allow',
      category: (res.category as string) ?? 'unknown',
      triggered,
      detections: detected,
    };
  }

  async submitBulkScan(
    profileName: string,
    prompts: string[],
    sessionId?: string,
  ): Promise<string[]> {
    const scanIds: string[] = [];

    for (let i = 0; i < prompts.length; i += BATCH_SIZE) {
      const batch = prompts.slice(i, i + BATCH_SIZE);
      const scanObjects = batch.map((prompt, idx) => ({
        req_id: i + idx,
        scan_req: {
          ai_profile: { profile_name: profileName },
          contents: [{ prompt }],
          ...(sessionId ? { session_id: sessionId } : {}),
        },
      }));

      const res = await this.scanner.asyncScan(scanObjects);
      scanIds.push(res.scan_id);
    }

    return scanIds;
  }

  /**
   * Poll async scan results until all complete or fail.
   *
   * Note: The async query API (`queryByScanIds`) does not return `prompt`,
   * `response`, `triggered`, or `detections` fields. These are set to
   * defaults (`''`, `undefined`, `false`, `{}`) in the returned results.
   * Use `scanPrompt()` (sync API) when these fields are needed.
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
        completed.set(id, {
          prompt: '', // not available from async API
          response: undefined, // not available from async API
          scanId: (result.scan_id as string) ?? id,
          reportId: (result.report_id as string) ?? '',
          action: result.action === 'block' ? 'block' : 'allow',
          category: (result.category as string) ?? 'unknown',
          triggered: false, // not available from async API — always false
          detections: {}, // not available from async API
        });
        pending.delete(id);
      } else if (status === 'failed') {
        completed.set(id, {
          prompt: '', // not available from async API
          response: undefined, // not available from async API
          scanId: id,
          reportId: '',
          action: 'allow', // safe default for failed scans
          category: 'error',
          triggered: false, // not available from async API — always false
          detections: {}, // not available from async API
        });
        pending.delete(id);
      }
    }
  }

  static formatResultsCsv(results: RuntimeScanResult[]): string {
    const header = 'prompt,action,category,triggered,scan_id,report_id';
    const rows = results.map((r) => {
      const escaped = r.prompt.replace(/"/g, '""');
      return `"${escaped}","${r.action}","${r.category}","${r.triggered}","${r.scanId}","${r.reportId}"`;
    });
    return [header, ...rows].join('\n');
  }
}
