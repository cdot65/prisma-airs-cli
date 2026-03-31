import { beforeEach, describe, expect, it, vi } from 'vitest';
import { SdkRuntimeService } from '../../../src/airs/runtime.js';

const mockScannerInstance = {
  syncScan: vi.fn(),
  asyncScan: vi.fn(),
  queryByScanIds: vi.fn(),
};

vi.mock('@cdot65/prisma-airs-sdk', () => ({
  init: vi.fn(),
  Scanner: vi.fn(() => mockScannerInstance),
  Content: vi.fn((opts: Record<string, string>) => opts),
}));

describe('SdkRuntimeService', () => {
  let service: SdkRuntimeService;

  beforeEach(() => {
    vi.clearAllMocks();
    service = new SdkRuntimeService('test-api-key');
  });

  describe('scanPrompt', () => {
    it('scans a prompt via syncScan and returns normalized result', async () => {
      mockScannerInstance.syncScan.mockResolvedValue({
        scan_id: 'scan-123',
        report_id: 'report-456',
        action: 'block',
        category: 'malicious',
        prompt_detected: { topic_violation: true, injection: false },
      });

      const result = await service.scanPrompt('my-profile', 'hack the system');

      expect(mockScannerInstance.syncScan).toHaveBeenCalledWith(
        { profile_name: 'my-profile' },
        { prompt: 'hack the system' },
        undefined,
      );
      expect(result).toEqual({
        prompt: 'hack the system',
        response: undefined,
        scanId: 'scan-123',
        reportId: 'report-456',
        action: 'block',
        category: 'malicious',
        triggered: true,
        detections: { topic_violation: true, injection: false },
      });
    });

    it('scans prompt with response when provided', async () => {
      mockScannerInstance.syncScan.mockResolvedValue({
        scan_id: 'scan-789',
        report_id: 'report-012',
        action: 'allow',
        category: 'benign',
        prompt_detected: {},
      });

      const result = await service.scanPrompt('my-profile', 'hello', 'world');

      expect(mockScannerInstance.syncScan).toHaveBeenCalledWith(
        { profile_name: 'my-profile' },
        { prompt: 'hello', response: 'world' },
        undefined,
      );
      expect(result.response).toBe('world');
      expect(result.action).toBe('allow');
    });

    it('handles missing detection fields gracefully', async () => {
      mockScannerInstance.syncScan.mockResolvedValue({
        scan_id: 'scan-1',
        report_id: 'report-1',
        action: 'allow',
        category: 'benign',
      });

      const result = await service.scanPrompt('p', 'test');
      expect(result.triggered).toBe(false);
      expect(result.detections).toEqual({});
    });
  });

  describe('submitBulkScan', () => {
    it('batches prompts into groups of 5 async scan objects', async () => {
      mockScannerInstance.asyncScan.mockResolvedValue({
        received: '2026-03-09T00:00:00Z',
        scan_id: 'batch-scan-1',
      });

      const prompts = Array.from({ length: 7 }, (_, i) => `prompt ${i}`);
      const scanIds = await service.submitBulkScan('my-profile', prompts);

      // 7 prompts → 2 batches (5 + 2)
      expect(mockScannerInstance.asyncScan).toHaveBeenCalledTimes(2);
      expect(scanIds).toHaveLength(2);

      // First batch: 5 items
      const firstCall = mockScannerInstance.asyncScan.mock.calls[0][0];
      expect(firstCall).toHaveLength(5);
      expect(firstCall[0].req_id).toBe(0);
      expect(firstCall[0].scan_req.ai_profile).toEqual({ profile_name: 'my-profile' });
      expect(firstCall[0].scan_req.contents).toEqual([{ prompt: 'prompt 0' }]);

      // Second batch: 2 items
      const secondCall = mockScannerInstance.asyncScan.mock.calls[1][0];
      expect(secondCall).toHaveLength(2);
    });

    it('passes session_id in scan_req when provided', async () => {
      mockScannerInstance.asyncScan.mockResolvedValue({
        received: '2026-03-09T00:00:00Z',
        scan_id: 'session-scan',
      });

      await service.submitBulkScan('my-profile', ['test prompt'], 'my-session-123');
      const scanObj = mockScannerInstance.asyncScan.mock.calls[0][0][0];
      expect(scanObj.scan_req.session_id).toBe('my-session-123');
    });

    it('omits session_id when not provided', async () => {
      mockScannerInstance.asyncScan.mockResolvedValue({
        received: '2026-03-09T00:00:00Z',
        scan_id: 'no-session',
      });

      await service.submitBulkScan('my-profile', ['test prompt']);
      const scanObj = mockScannerInstance.asyncScan.mock.calls[0][0][0];
      expect(scanObj.scan_req.session_id).toBeUndefined();
    });

    it('handles single prompt', async () => {
      mockScannerInstance.asyncScan.mockResolvedValue({
        received: '2026-03-09T00:00:00Z',
        scan_id: 'single-scan',
      });

      const scanIds = await service.submitBulkScan('p', ['one prompt']);
      expect(mockScannerInstance.asyncScan).toHaveBeenCalledTimes(1);
      expect(scanIds).toEqual(['single-scan']);
    });

    it('handles exactly 5 prompts in one batch', async () => {
      mockScannerInstance.asyncScan.mockResolvedValue({
        received: '2026-03-09T00:00:00Z',
        scan_id: 'exact-5',
      });

      const prompts = Array.from({ length: 5 }, (_, i) => `p${i}`);
      await service.submitBulkScan('p', prompts);
      expect(mockScannerInstance.asyncScan).toHaveBeenCalledTimes(1);
      expect(mockScannerInstance.asyncScan.mock.calls[0][0]).toHaveLength(5);
    });
  });

  describe('pollResults', () => {
    it('polls until all scans complete', async () => {
      mockScannerInstance.queryByScanIds
        .mockResolvedValueOnce([
          { scan_id: 's1', status: 'PENDING' },
          {
            scan_id: 's2',
            status: 'COMPLETED',
            result: { scan_id: 's2', report_id: 'r2', action: 'allow', category: 'benign' },
          },
        ])
        .mockResolvedValueOnce([
          {
            scan_id: 's1',
            status: 'COMPLETED',
            result: { scan_id: 's1', report_id: 'r1', action: 'block', category: 'malicious' },
          },
        ]);

      const results = await service.pollResults(['s1', 's2'], 10);

      expect(results).toHaveLength(2);
      expect(results.find((r) => r.scanId === 's1')?.action).toBe('block');
      expect(results.find((r) => r.scanId === 's2')?.action).toBe('allow');
    });

    it('handles FAILED scans', async () => {
      mockScannerInstance.queryByScanIds.mockResolvedValueOnce([
        { scan_id: 's1', status: 'FAILED' },
      ]);

      const results = await service.pollResults(['s1'], 10);
      expect(results).toHaveLength(1);
      expect(results[0].action).toBe('allow');
      expect(results[0].category).toBe('error');
    });
  });

  describe('pollResults — lowercase API statuses', () => {
    it('handles lowercase "complete" from API', async () => {
      mockScannerInstance.queryByScanIds.mockResolvedValueOnce([
        {
          scan_id: 's1',
          status: 'complete',
          result: { scan_id: 's1', report_id: 'r1', action: 'allow', category: 'benign' },
        },
      ]);

      const results = await service.pollResults(['s1'], 10);
      expect(results).toHaveLength(1);
      expect(results[0].action).toBe('allow');
      expect(results[0].category).toBe('benign');
    });

    it('handles lowercase "failed" from API', async () => {
      mockScannerInstance.queryByScanIds.mockResolvedValueOnce([
        { scan_id: 's1', status: 'failed' },
      ]);

      const results = await service.pollResults(['s1'], 10);
      expect(results).toHaveLength(1);
      expect(results[0].action).toBe('allow');
      expect(results[0].category).toBe('error');
    });

    it('handles mixed casing in single poll response', async () => {
      mockScannerInstance.queryByScanIds.mockResolvedValueOnce([
        {
          scan_id: 's1',
          status: 'complete',
          result: { scan_id: 's1', report_id: 'r1', action: 'block', category: 'malicious' },
        },
        { scan_id: 's2', status: 'failed' },
      ]);

      const results = await service.pollResults(['s1', 's2'], 10);
      expect(results).toHaveLength(2);
      expect(results[0].action).toBe('block');
      expect(results[1].category).toBe('error');
    });

    it('treats lowercase "pending" as not-yet-complete and re-polls', async () => {
      mockScannerInstance.queryByScanIds
        .mockResolvedValueOnce([{ scan_id: 's1', status: 'pending' }])
        .mockResolvedValueOnce([
          {
            scan_id: 's1',
            status: 'complete',
            result: { scan_id: 's1', report_id: 'r1', action: 'allow', category: 'benign' },
          },
        ]);

      const results = await service.pollResults(['s1'], 10);
      expect(results).toHaveLength(1);
      expect(results[0].action).toBe('allow');
      expect(mockScannerInstance.queryByScanIds).toHaveBeenCalledTimes(2);
    });
  });

  describe('submitBulkScan — edge cases', () => {
    it('returns empty array for empty prompts', async () => {
      const scanIds = await service.submitBulkScan('p', []);
      expect(mockScannerInstance.asyncScan).not.toHaveBeenCalled();
      expect(scanIds).toEqual([]);
    });

    it('exactly 5 prompts creates 1 batch (not 2)', async () => {
      mockScannerInstance.asyncScan.mockResolvedValue({
        received: '2026-03-09T00:00:00Z',
        scan_id: 'batch-5',
      });

      const prompts = Array.from({ length: 5 }, (_, i) => `p${i}`);
      const scanIds = await service.submitBulkScan('profile', prompts);
      expect(mockScannerInstance.asyncScan).toHaveBeenCalledTimes(1);
      expect(scanIds).toHaveLength(1);
      expect(mockScannerInstance.asyncScan.mock.calls[0][0]).toHaveLength(5);
    });

    it('6 prompts creates 2 batches (5 + 1)', async () => {
      mockScannerInstance.asyncScan
        .mockResolvedValueOnce({ received: '2026-03-09T00:00:00Z', scan_id: 'batch-a' })
        .mockResolvedValueOnce({ received: '2026-03-09T00:00:00Z', scan_id: 'batch-b' });

      const prompts = Array.from({ length: 6 }, (_, i) => `p${i}`);
      const scanIds = await service.submitBulkScan('profile', prompts);
      expect(mockScannerInstance.asyncScan).toHaveBeenCalledTimes(2);
      expect(scanIds).toEqual(['batch-a', 'batch-b']);
      expect(mockScannerInstance.asyncScan.mock.calls[0][0]).toHaveLength(5);
      expect(mockScannerInstance.asyncScan.mock.calls[1][0]).toHaveLength(1);
    });
  });

  describe('pollResults — edge cases', () => {
    it('handles mix of COMPLETED and FAILED statuses in single poll', async () => {
      mockScannerInstance.queryByScanIds.mockResolvedValueOnce([
        {
          scan_id: 's1',
          status: 'COMPLETED',
          result: { scan_id: 's1', report_id: 'r1', action: 'block', category: 'malicious' },
        },
        { scan_id: 's2', status: 'FAILED' },
        {
          scan_id: 's3',
          status: 'COMPLETED',
          result: { scan_id: 's3', report_id: 'r3', action: 'allow', category: 'benign' },
        },
      ]);

      const results = await service.pollResults(['s1', 's2', 's3'], 10);
      expect(results).toHaveLength(3);
      expect(results[0].action).toBe('block');
      expect(results[1].action).toBe('allow');
      expect(results[1].category).toBe('error');
      expect(results[2].action).toBe('allow');
      expect(results[2].category).toBe('benign');
    });
  });

  describe('pollResults — rate limit retry', () => {
    it('retries on rate limit error and succeeds', async () => {
      const rateLimitError = new Error('Rate limit exceeded');

      mockScannerInstance.queryByScanIds
        .mockRejectedValueOnce(rateLimitError)
        .mockRejectedValueOnce(rateLimitError)
        .mockResolvedValueOnce([
          {
            scan_id: 's1',
            status: 'COMPLETED',
            result: { scan_id: 's1', report_id: 'r1', action: 'allow', category: 'benign' },
          },
        ]);

      const results = await service.pollResults(['s1'], 10, { baseDelayMs: 10 });
      expect(results).toHaveLength(1);
      expect(results[0].action).toBe('allow');
      expect(mockScannerInstance.queryByScanIds).toHaveBeenCalledTimes(3);
    });

    it('throws after exhausting max retries', async () => {
      const rateLimitError = new Error('Rate limit exceeded');
      // Need enough rejections to exceed maxRetries (3 retries + 1 initial = 4 calls)
      for (let i = 0; i < 4; i++) {
        mockScannerInstance.queryByScanIds.mockRejectedValueOnce(rateLimitError);
      }

      await expect(
        service.pollResults(['s1'], 10, { maxRetries: 3, baseDelayMs: 10 }),
      ).rejects.toThrow('Rate limit exceeded');
      expect(mockScannerInstance.queryByScanIds).toHaveBeenCalledTimes(4);
    });

    it('does not retry on non-rate-limit errors', async () => {
      const otherError = new Error('Network timeout');
      mockScannerInstance.queryByScanIds.mockRejectedValueOnce(otherError);

      await expect(service.pollResults(['s1'], 10)).rejects.toThrow('Network timeout');
      expect(mockScannerInstance.queryByScanIds).toHaveBeenCalledTimes(1);
    });

    it('calls onRetry callback when retrying', async () => {
      const rateLimitError = new Error('Rate limit exceeded');
      const onRetry = vi.fn();

      mockScannerInstance.queryByScanIds
        .mockRejectedValueOnce(rateLimitError)
        .mockResolvedValueOnce([
          {
            scan_id: 's1',
            status: 'COMPLETED',
            result: { scan_id: 's1', report_id: 'r1', action: 'block', category: 'malicious' },
          },
        ]);

      await service.pollResults(['s1'], 10, { baseDelayMs: 10, onRetry });
      expect(onRetry).toHaveBeenCalledTimes(1);
      expect(onRetry).toHaveBeenCalledWith(1, expect.any(Number));
    });

    it('decays retry level after a full successful sweep, not per-batch', async () => {
      const rateLimitError = new Error('Rate limit exceeded');
      const onRetry = vi.fn();

      // Single scan ID — each sweep = 1 batch
      // Rate limit twice → level escalates to 2
      // Then full sweep succeeds (PENDING) → decay to 1
      // Then full sweep succeeds (COMPLETE) → decay to 0, done
      mockScannerInstance.queryByScanIds
        .mockRejectedValueOnce(rateLimitError) // retry 1
        .mockRejectedValueOnce(rateLimitError) // retry 2
        .mockResolvedValueOnce([{ scan_id: 's1', status: 'PENDING' }]) // sweep ok, decay 2→1
        .mockResolvedValueOnce([
          {
            scan_id: 's1',
            status: 'COMPLETED',
            result: { scan_id: 's1', report_id: 'r1', action: 'allow', category: 'benign' },
          },
        ]); // sweep ok, decay 1→0

      const results = await service.pollResults(['s1'], 10, {
        maxRetries: 5,
        baseDelayMs: 10,
        onRetry,
      });
      expect(results).toHaveLength(1);
      expect(onRetry).toHaveBeenCalledTimes(2);
      expect(onRetry.mock.calls[0][0]).toBe(1);
      expect(onRetry.mock.calls[1][0]).toBe(2);
    });

    it('does not decay retry level when early batches succeed but later ones fail', async () => {
      const rateLimitError = new Error('Rate limit exceeded');
      const onRetry = vi.fn();

      // 10 scan IDs = 2 batches per sweep
      const ids = ['s0', 's1', 's2', 's3', 's4', 's5', 's6', 's7', 's8', 's9'];

      // Sweep 1: batch 1 succeeds (all pending), batch 2 rate-limits → level 1
      mockScannerInstance.queryByScanIds
        .mockResolvedValueOnce(ids.slice(0, 5).map((id) => ({ scan_id: id, status: 'PENDING' })))
        .mockRejectedValueOnce(rateLimitError) // level → 1
        // Sweep 2: batch 1 succeeds (all pending), batch 2 rate-limits → level 2
        .mockResolvedValueOnce(ids.slice(0, 5).map((id) => ({ scan_id: id, status: 'PENDING' })))
        .mockRejectedValueOnce(rateLimitError) // level → 2
        // Sweep 3: both batches succeed → all complete
        .mockResolvedValueOnce(
          ids.slice(0, 5).map((id) => ({
            scan_id: id,
            status: 'COMPLETED',
            result: { scan_id: id, report_id: `r-${id}`, action: 'allow', category: 'benign' },
          })),
        )
        .mockResolvedValueOnce(
          ids.slice(5).map((id) => ({
            scan_id: id,
            status: 'COMPLETED',
            result: { scan_id: id, report_id: `r-${id}`, action: 'allow', category: 'benign' },
          })),
        );

      const results = await service.pollResults(ids, 10, {
        maxRetries: 5,
        baseDelayMs: 10,
        onRetry,
      });

      expect(results).toHaveLength(10);
      // Key assertion: retry must escalate to 2, not stay at 1
      expect(onRetry).toHaveBeenCalledTimes(2);
      expect(onRetry.mock.calls[0][0]).toBe(1);
      expect(onRetry.mock.calls[1][0]).toBe(2); // must be 2, not 1
    });

    it('queries all pending IDs per sweep in batches of 5', async () => {
      // 12 scan IDs = 3 batches of 5,5,2 per sweep
      const ids = Array.from({ length: 12 }, (_, i) => `s${i}`);

      // All complete on first sweep (3 batch queries)
      for (let i = 0; i < 12; i += 5) {
        const batch = ids.slice(i, i + 5);
        mockScannerInstance.queryByScanIds.mockResolvedValueOnce(
          batch.map((id) => ({
            scan_id: id,
            status: 'COMPLETED',
            result: { scan_id: id, report_id: `r-${id}`, action: 'allow', category: 'benign' },
          })),
        );
      }

      const results = await service.pollResults(ids, 10, { baseDelayMs: 10 });
      expect(results).toHaveLength(12);
      // Should have made 3 batch queries in one sweep
      expect(mockScannerInstance.queryByScanIds).toHaveBeenCalledTimes(3);
    });
  });

  describe('formatResultsCsv', () => {
    it('produces CSV with header and data rows', () => {
      const results = [
        {
          prompt: 'hello',
          response: undefined,
          scanId: 's1',
          reportId: 'r1',
          action: 'allow' as const,
          category: 'benign',
          triggered: false,
          detections: {},
        },
        {
          prompt: 'hack it',
          response: undefined,
          scanId: 's2',
          reportId: 'r2',
          action: 'block' as const,
          category: 'malicious',
          triggered: true,
          detections: { injection: true },
        },
      ];

      const csv = SdkRuntimeService.formatResultsCsv(results);
      const lines = csv.split('\n');
      expect(lines[0]).toBe('prompt,action,category,triggered,scan_id,report_id');
      expect(lines[1]).toBe('"hello","allow","benign","false","s1","r1"');
      expect(lines[2]).toBe('"hack it","block","malicious","true","s2","r2"');
    });

    it('escapes quotes in prompt text', () => {
      const results = [
        {
          prompt: 'say "hello"',
          response: undefined,
          scanId: 's1',
          reportId: 'r1',
          action: 'allow' as const,
          category: 'benign',
          triggered: false,
          detections: {},
        },
      ];

      const csv = SdkRuntimeService.formatResultsCsv(results);
      expect(csv).toContain('"say ""hello"""');
    });

    it('handles prompts with commas (CSV escaping)', () => {
      const results = [
        {
          prompt: 'hello, world, test',
          response: undefined,
          scanId: 's1',
          reportId: 'r1',
          action: 'allow' as const,
          category: 'benign',
          triggered: false,
          detections: {},
        },
      ];

      const csv = SdkRuntimeService.formatResultsCsv(results);
      const lines = csv.split('\n');
      // prompt is wrapped in quotes so commas don't break CSV parsing
      expect(lines[1]).toBe('"hello, world, test","allow","benign","false","s1","r1"');
    });

    it('returns header only for empty results array', () => {
      const csv = SdkRuntimeService.formatResultsCsv([]);
      expect(csv).toBe('prompt,action,category,triggered,scan_id,report_id');
    });
  });
});
