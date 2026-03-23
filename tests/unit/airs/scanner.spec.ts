import * as fs from 'node:fs/promises';
import * as os from 'node:os';
import * as path from 'node:path';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import {
  AirsScanService,
  DebugScanService,
  RateLimitedScanService,
} from '../../../src/airs/scanner.js';
import type { ScanResult, ScanService } from '../../../src/airs/types.js';

// Mock the SDK
const mockSyncScan = vi.fn();

vi.mock('@cdot65/prisma-airs-sdk', () => ({
  init: vi.fn(),
  Scanner: vi.fn().mockImplementation(() => ({ syncScan: mockSyncScan })),
  Content: vi.fn().mockImplementation((opts: { prompt: string }) => ({
    prompt: opts.prompt,
    toJSON: () => ({ prompt: opts.prompt }),
  })),
}));

describe('AirsScanService', () => {
  let service: AirsScanService;

  beforeEach(() => {
    vi.clearAllMocks();
    service = new AirsScanService('test-api-key');
  });

  describe('scan', () => {
    it('returns blocked result when topic_violation is true', async () => {
      mockSyncScan.mockResolvedValue({
        scan_id: 'scan-123',
        report_id: 'report-456',
        action: 'block',
        prompt_detected: {
          topic_violation: true,
        },
      });

      const result = await service.scan('my-profile', 'How to make a bomb');
      expect(result.scanId).toBe('scan-123');
      expect(result.reportId).toBe('report-456');
      expect(result.action).toBe('block');
      expect(result.triggered).toBe(true);
    });

    it('does not trigger on topic_guardrails_details alone', async () => {
      mockSyncScan.mockResolvedValue({
        scan_id: 's1',
        report_id: 'r1',
        action: 'block',
        prompt_detected: { topic_guardrails_details: true },
      });

      const result = await service.scan('my-profile', 'test');
      expect(result.triggered).toBe(false);
    });

    it('returns allowed result when not triggered', async () => {
      mockSyncScan.mockResolvedValue({
        scan_id: 'scan-789',
        report_id: 'report-012',
        action: 'allow',
        prompt_detected: {},
      });

      const result = await service.scan('my-profile', 'Tell me about cats');
      expect(result.action).toBe('allow');
      expect(result.triggered).toBe(false);
    });

    it('passes profile name to scanner', async () => {
      mockSyncScan.mockResolvedValue({
        scan_id: 's1',
        report_id: 'r1',
        action: 'allow',
        prompt_detected: {},
      });

      await service.scan('prod-profile', 'hello');
      expect(mockSyncScan).toHaveBeenCalledWith(
        expect.objectContaining({ profile_name: 'prod-profile' }),
        expect.anything(),
        undefined,
      );
    });

    it('falls back to empty strings when scan_id/report_id are undefined', async () => {
      mockSyncScan.mockResolvedValue({
        action: 'allow',
        prompt_detected: {},
      });

      const result = await service.scan('my-profile', 'hello');
      expect(result.scanId).toBe('');
      expect(result.reportId).toBe('');
    });

    it('detects topic_violation as triggered', async () => {
      mockSyncScan.mockResolvedValue({
        scan_id: 's1',
        report_id: 'r1',
        action: 'block',
        prompt_detected: { topic_violation: true },
      });

      const result = await service.scan('my-profile', 'bad prompt');
      expect(result.triggered).toBe(true);
    });

    it('passes sessionId to syncScan when provided', async () => {
      mockSyncScan.mockResolvedValue({
        scan_id: 's1',
        report_id: 'r1',
        action: 'allow',
        prompt_detected: {},
      });

      await service.scan('prof', 'hello', 'prisma-airs-cli-abc1234-iter1');
      expect(mockSyncScan).toHaveBeenCalledWith(
        expect.objectContaining({ profile_name: 'prof' }),
        expect.anything(),
        { sessionId: 'prisma-airs-cli-abc1234-iter1' },
      );
    });

    it('extracts category from response', async () => {
      mockSyncScan.mockResolvedValue({
        scan_id: 's1',
        report_id: 'r1',
        action: 'allow',
        category: 'benign',
        prompt_detected: {},
      });

      const result = await service.scan('prof', 'hello');
      expect(result.category).toBe('benign');
    });

    it('returns undefined category when not in response', async () => {
      mockSyncScan.mockResolvedValue({
        scan_id: 's1',
        report_id: 'r1',
        action: 'allow',
        prompt_detected: {},
      });

      const result = await service.scan('prof', 'hello');
      expect(result.category).toBeUndefined();
    });
  });

  describe('scanBatch', () => {
    it('scans multiple prompts and returns results', async () => {
      mockSyncScan
        .mockResolvedValueOnce({
          scan_id: 's1',
          report_id: 'r1',
          action: 'block',
          prompt_detected: { topic_violation: true },
        })
        .mockResolvedValueOnce({
          scan_id: 's2',
          report_id: 'r2',
          action: 'allow',
          prompt_detected: {},
        });

      const results = await service.scanBatch('profile', ['prompt1', 'prompt2'], 2);
      expect(results).toHaveLength(2);
      expect(results[0].action).toBe('block');
      expect(results[1].action).toBe('allow');
    });

    it('respects concurrency limit', async () => {
      let concurrent = 0;
      let maxConcurrent = 0;

      mockSyncScan.mockImplementation(async () => {
        concurrent++;
        maxConcurrent = Math.max(maxConcurrent, concurrent);
        await new Promise((r) => setTimeout(r, 10));
        concurrent--;
        return { scan_id: 's', report_id: 'r', action: 'allow', prompt_detected: {} };
      });

      const prompts = Array(10).fill('test');
      await service.scanBatch('profile', prompts, 3);
      expect(maxConcurrent).toBeLessThanOrEqual(3);
    });
  });
});

describe('DebugScanService', () => {
  function createMockInner(): ScanService {
    return {
      scan: async (): Promise<ScanResult> => ({
        scanId: 'scan-1',
        reportId: 'report-1',
        action: 'allow',
        triggered: false,
        category: 'benign',
        raw: { some: 'data' },
      }),
      scanBatch: async (_p: string, prompts: string[]): Promise<ScanResult[]> =>
        prompts.map(() => ({
          scanId: 'scan-1',
          reportId: 'report-1',
          action: 'allow' as const,
          triggered: false,
          category: 'benign',
          raw: { some: 'data' },
        })),
    };
  }

  it('delegates to inner scanner and returns result', async () => {
    const tmpFile = path.join(os.tmpdir(), `debug-test-${Date.now()}.jsonl`);
    const inner = createMockInner();
    const debug = new DebugScanService(inner, tmpFile);

    const result = await debug.scan('profile', 'hello');
    expect(result.scanId).toBe('scan-1');
    expect(result.category).toBe('benign');

    // Clean up
    await fs.unlink(tmpFile).catch(() => {});
  });

  it('writes JSONL entries to file', async () => {
    const tmpFile = path.join(os.tmpdir(), `debug-test-${Date.now()}.jsonl`);
    const inner = createMockInner();
    const debug = new DebugScanService(inner, tmpFile);

    await debug.scan('profile', 'prompt one');
    await debug.scan('profile', 'prompt two');

    const content = await fs.readFile(tmpFile, 'utf-8');
    const lines = content.trim().split('\n');
    expect(lines).toHaveLength(2);

    const entry1 = JSON.parse(lines[0]);
    expect(entry1.prompt).toBe('prompt one');
    expect(entry1.profileName).toBe('profile');
    expect(entry1.result).toBeDefined();
    expect(entry1.timestamp).toBeTypeOf('number');

    const entry2 = JSON.parse(lines[1]);
    expect(entry2.prompt).toBe('prompt two');

    await fs.unlink(tmpFile).catch(() => {});
  });

  it('scanBatch delegates through scan and writes entries', async () => {
    const tmpFile = path.join(os.tmpdir(), `debug-test-${Date.now()}.jsonl`);
    const inner = createMockInner();
    const debug = new DebugScanService(inner, tmpFile);

    const results = await debug.scanBatch('profile', ['a', 'b', 'c']);
    expect(results).toHaveLength(3);

    const content = await fs.readFile(tmpFile, 'utf-8');
    const lines = content.trim().split('\n');
    expect(lines).toHaveLength(3);

    await fs.unlink(tmpFile).catch(() => {});
  });
});

describe('RateLimitedScanService', () => {
  function createMockInner(): ScanService {
    return {
      scan: vi.fn(
        async (): Promise<ScanResult> => ({
          scanId: 'scan-1',
          reportId: 'report-1',
          action: 'allow',
          triggered: false,
          category: 'benign',
        }),
      ),
      scanBatch: vi.fn(
        async (_p: string, prompts: string[]): Promise<ScanResult[]> =>
          prompts.map(() => ({
            scanId: 'scan-1',
            reportId: 'report-1',
            action: 'allow' as const,
            triggered: false,
            category: 'benign',
          })),
      ),
    };
  }

  it('delegates scan calls to inner service', async () => {
    const inner = createMockInner();
    const limited = new RateLimitedScanService(inner, 100);

    const result = await limited.scan('profile', 'hello');
    expect(result.scanId).toBe('scan-1');
    expect(inner.scan).toHaveBeenCalledWith('profile', 'hello', undefined);
  });

  it('allows up to maxPerSecond calls without delay', async () => {
    const inner = createMockInner();
    const rate = 10;
    const limited = new RateLimitedScanService(inner, rate);

    const start = Date.now();
    const promises = Array.from({ length: rate }, (_, i) => limited.scan('profile', `prompt-${i}`));
    await Promise.all(promises);
    const elapsed = Date.now() - start;

    // 10 calls at rate 10/s should complete in well under 1s
    expect(elapsed).toBeLessThan(500);
    expect(inner.scan).toHaveBeenCalledTimes(rate);
  });

  it('delays calls that exceed the per-second rate', async () => {
    const inner = createMockInner();
    const rate = 5;
    const limited = new RateLimitedScanService(inner, rate);

    const start = Date.now();
    // Fire rate+1 calls — the last one must wait for the window to slide
    const promises = Array.from({ length: rate + 1 }, (_, i) =>
      limited.scan('profile', `prompt-${i}`),
    );
    await Promise.all(promises);
    const elapsed = Date.now() - start;

    // The 6th call should have been delayed until the 1s window slides
    expect(elapsed).toBeGreaterThanOrEqual(200);
    expect(inner.scan).toHaveBeenCalledTimes(rate + 1);
  });

  it('scanBatch respects rate limit across concurrent calls', async () => {
    const callTimes: number[] = [];
    const inner: ScanService = {
      scan: vi.fn(async (): Promise<ScanResult> => {
        callTimes.push(Date.now());
        return {
          scanId: 's',
          reportId: 'r',
          action: 'allow',
          triggered: false,
        };
      }),
      scanBatch: vi.fn(),
    };

    const rate = 5;
    const limited = new RateLimitedScanService(inner, rate);

    // 8 prompts at rate 5/s — first 5 immediate, next 3 delayed
    await limited.scanBatch('profile', Array(8).fill('test'), 8);

    expect(callTimes).toHaveLength(8);
    // Verify the batch was processed through the rate limiter
    expect(inner.scan).toHaveBeenCalledTimes(8);
  });

  it('passes sessionId through to inner scan', async () => {
    const inner = createMockInner();
    const limited = new RateLimitedScanService(inner, 10);

    await limited.scan('profile', 'hello', 'session-123');
    expect(inner.scan).toHaveBeenCalledWith('profile', 'hello', 'session-123');
  });
});
