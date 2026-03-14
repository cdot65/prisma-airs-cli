import * as fs from 'node:fs/promises';
import { Content, init, Scanner } from '@cdot65/prisma-airs-sdk';
import pLimit from 'p-limit';
import type { ScanResult, ScanService } from './types.js';

/** Scans prompts against AIRS security profiles via the Prisma AIRS SDK. */
export class AirsScanService implements ScanService {
  private scanner: InstanceType<typeof Scanner>;

  constructor(apiKey: string) {
    init({ apiKey });
    this.scanner = new Scanner();
  }

  /** Scan a single prompt synchronously and return the normalized result. */
  async scan(profileName: string, prompt: string, sessionId?: string): Promise<ScanResult> {
    const content = new Content({ prompt });
    const response = await this.scanner.syncScan(
      { profile_name: profileName },
      content,
      sessionId ? { sessionId } : undefined,
    );

    const action = response.action === 'block' ? 'block' : 'allow';
    const detected = response.prompt_detected as Record<string, unknown> | undefined;
    const triggered = detected?.topic_violation === true;
    const category = (response.category as string) ?? undefined;

    return {
      scanId: response.scan_id ?? '',
      reportId: response.report_id ?? '',
      action: action as 'allow' | 'block',
      triggered,
      category,
      raw: response,
    };
  }

  /** Scan multiple prompts concurrently (default 5) and return results in order. */
  async scanBatch(
    profileName: string,
    prompts: string[],
    concurrency = 5,
    sessionId?: string,
  ): Promise<ScanResult[]> {
    const limit = pLimit(concurrency);
    return Promise.all(
      prompts.map((prompt) => limit(() => this.scan(profileName, prompt, sessionId))),
    );
  }
}

/**
 * Debug wrapper that delegates to an inner ScanService and appends
 * each raw response + prompt to a JSONL file for offline inspection.
 */
export class DebugScanService implements ScanService {
  constructor(
    private inner: ScanService,
    private filePath: string,
  ) {}

  async scan(profileName: string, prompt: string, sessionId?: string): Promise<ScanResult> {
    const result = await this.inner.scan(profileName, prompt, sessionId);
    const entry = JSON.stringify({
      prompt,
      profileName,
      result,
      raw: result.raw,
      timestamp: Date.now(),
    });
    await fs.appendFile(this.filePath, `${entry}\n`);
    return result;
  }

  async scanBatch(
    profileName: string,
    prompts: string[],
    concurrency?: number,
    sessionId?: string,
  ): Promise<ScanResult[]> {
    const limit = pLimit(concurrency ?? 5);
    return Promise.all(
      prompts.map((prompt) => limit(() => this.scan(profileName, prompt, sessionId))),
    );
  }
}
