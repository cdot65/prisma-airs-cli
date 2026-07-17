import { beforeEach, describe, expect, it, vi } from 'vitest';
import { SdkRuntimeService } from '../../../src/airs/runtime.js';

const mockScannerInstance = {
  syncScan: vi.fn(),
  asyncScan: vi.fn(),
  queryByScanIds: vi.fn(),
  queryByReportIds: vi.fn(),
};

vi.mock('@cdot65/prisma-airs-sdk', () => ({
  init: vi.fn(),
  MAX_NUMBER_OF_BATCH_SCAN_OBJECTS: 20,
  Scanner: vi.fn(() => mockScannerInstance),
  Content: vi.fn((opts: Record<string, string>) => opts),
}));

describe('SdkRuntimeService', () => {
  let service: SdkRuntimeService;

  beforeEach(() => {
    vi.clearAllMocks();
    mockScannerInstance.syncScan.mockReset();
    mockScannerInstance.asyncScan.mockReset();
    mockScannerInstance.queryByScanIds.mockReset();
    mockScannerInstance.queryByReportIds.mockReset();
    service = new SdkRuntimeService({ apiKey: 'test-api-key' });
  });

  describe('constructor', () => {
    it('passes all init options through to SDK init()', async () => {
      const { init } = await import('@cdot65/prisma-airs-sdk');
      new SdkRuntimeService({
        apiKey: 'k',
        apiToken: 't',
        apiEndpoint: 'https://airs.example.com',
        numRetries: 2,
      });
      expect(init).toHaveBeenCalledWith({
        apiKey: 'k',
        apiToken: 't',
        apiEndpoint: 'https://airs.example.com',
        numRetries: 2,
      });
    });

    it('supports apiToken-only auth', async () => {
      const { init } = await import('@cdot65/prisma-airs-sdk');
      new SdkRuntimeService({ apiToken: 'tok-only' });
      expect(init).toHaveBeenCalledWith(expect.objectContaining({ apiToken: 'tok-only' }));
    });
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

    it('includes source-code and agent detectors on the compatibility result', async () => {
      mockScannerInstance.syncScan.mockResolvedValue({
        scan_id: 'scan-new-detectors',
        report_id: 'report-new-detectors',
        action: 'block',
        category: 'suspicious',
        timeout: false,
        error: false,
        errors: [],
        prompt_detected: { source_code: true, agent: true },
      });

      const result = await service.scanPrompt('profile', 'prompt');

      expect(result.action).toBe('block');
      expect(result.triggered).toBe(true);
      expect(result.detections).toEqual({ source_code: true, agent: true });
    });
  });

  describe('submitBulkScan', () => {
    it('batches prompts into groups of 20 async scan objects', async () => {
      mockScannerInstance.asyncScan.mockResolvedValue({
        received: '2026-03-09T00:00:00Z',
        scan_id: 'batch-scan-1',
      });

      const prompts = Array.from({ length: 21 }, (_, i) => `prompt ${i}`);
      const scanIds = await service.submitBulkScan('my-profile', prompts);

      // 21 prompts → 2 batches (20 + 1)
      expect(mockScannerInstance.asyncScan).toHaveBeenCalledTimes(2);
      expect(scanIds).toHaveLength(2);

      // First batch: 20 items
      const firstCall = mockScannerInstance.asyncScan.mock.calls[0][0];
      expect(firstCall).toHaveLength(20);
      expect(firstCall[0].req_id).toBe(0);
      expect(firstCall[0].scan_req.ai_profile).toEqual({ profile_name: 'my-profile' });
      expect(firstCall[0].scan_req.contents).toEqual([{ prompt: 'prompt 0' }]);

      // Second batch: 1 item
      const secondCall = mockScannerInstance.asyncScan.mock.calls[1][0];
      expect(secondCall).toHaveLength(1);
      expect(
        mockScannerInstance.asyncScan.mock.calls.every((call) => call[1]?.numRetries === 0),
      ).toBe(true);
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

  describe('submitBatch', () => {
    it('rejects duplicate prompt indices before submitting', async () => {
      await expect(
        service.submitBatch('profile', [
          { index: 0, prompt: 'first' },
          { index: 0, prompt: 'duplicate' },
        ]),
      ).rejects.toThrow(/unique.*index/i);
      expect(mockScannerInstance.asyncScan).not.toHaveBeenCalled();
    });

    it('accepts 20 prompts and rejects 21 before submitting', async () => {
      mockScannerInstance.asyncScan.mockResolvedValueOnce({
        received: '2026-07-17T00:00:00Z',
        scan_id: 'batch-20',
      });
      const twenty = Array.from({ length: 20 }, (_, index) => ({
        index,
        prompt: `prompt ${index}`,
      }));

      const receipt = await service.submitBatch('profile', twenty);
      expect(receipt.entries).toHaveLength(20);
      expect(mockScannerInstance.asyncScan.mock.calls[0][0]).toHaveLength(20);

      await expect(
        service.submitBatch('profile', [...twenty, { index: 20, prompt: 'one too many' }]),
      ).rejects.toThrow(/between 1 and 20 prompts/i);
      expect(mockScannerInstance.asyncScan).toHaveBeenCalledOnce();
    });

    it('disables hidden SDK retries for the ambiguous async POST', async () => {
      mockScannerInstance.asyncScan.mockResolvedValueOnce({
        received: '2026-07-17T00:00:00Z',
        scan_id: 'single-attempt',
      });

      await service.submitBatch('profile', [{ index: 0, prompt: 'one prompt' }]);

      expect(mockScannerInstance.asyncScan).toHaveBeenCalledWith(
        [
          {
            req_id: 0,
            scan_req: {
              ai_profile: { profile_name: 'profile' },
              contents: [{ prompt: 'one prompt' }],
            },
          },
        ],
        { numRetries: 0 },
      );
    });

    it('retries a definite 429 using the SDK retry delay without enabling POST retries', async () => {
      const rateLimit = Object.assign(new Error('rate limited'), {
        failureKind: 'http',
        statusCode: 429,
        retryAfterMs: 0,
      });
      const onRetry = vi.fn();
      mockScannerInstance.asyncScan.mockRejectedValueOnce(rateLimit).mockResolvedValueOnce({
        received: '2026-07-17T00:00:00Z',
        scan_id: 'after-rate-limit',
      });

      const receipt = await service.submitBatch(
        'profile',
        [{ index: 0, prompt: 'one prompt' }],
        undefined,
        { baseDelayMs: 1, onRetry },
      );

      expect(receipt.scanId).toBe('after-rate-limit');
      expect(mockScannerInstance.asyncScan).toHaveBeenCalledTimes(2);
      expect(
        mockScannerInstance.asyncScan.mock.calls.every((call) => call[1].numRetries === 0),
      ).toBe(true);
      expect(onRetry).toHaveBeenCalledWith(1, 0);
    });

    it('does not retry an ambiguous network failure even when its message mentions 429', async () => {
      const networkFailure = Object.assign(new Error('socket closed after upstream 429'), {
        failureKind: 'network',
      });
      mockScannerInstance.asyncScan.mockRejectedValueOnce(networkFailure).mockResolvedValueOnce({
        received: '2026-07-17T00:00:00Z',
        scan_id: 'unsafe-retry',
      });

      await expect(
        service.submitBatch('profile', [{ index: 0, prompt: 'one prompt' }], undefined, {
          baseDelayMs: 0,
        }),
      ).rejects.toBe(networkFailure);
      expect(mockScannerInstance.asyncScan).toHaveBeenCalledOnce();
    });
  });

  describe('pollBatch', () => {
    it('rejects receipt entries that do not belong to the batch scan ID', async () => {
      await expect(
        service.pollBatch(
          {
            scanId: 'expected-scan',
            entries: [{ scanId: 'different-scan', reqId: 0, index: 0, prompt: 'prompt' }],
          },
          0,
        ),
      ).rejects.toThrow(/entry.*scan ID/i);
      expect(mockScannerInstance.queryByScanIds).not.toHaveBeenCalled();
    });

    it('falls back to report rows when a terminal scan row has no request ID', async () => {
      mockScannerInstance.queryByScanIds.mockResolvedValueOnce([
        {
          scan_id: 'batch-fallback',
          status: 'complete',
          result: { scan_id: 'batch-fallback', report_id: 'report-fallback' },
        },
      ]);
      mockScannerInstance.queryByReportIds.mockResolvedValueOnce([
        {
          scan_id: 'batch-fallback',
          report_id: 'report-fallback',
          req_id: 1,
          detection_results: [{ detection_service: 'pi', verdict: 'benign', action: 'allow' }],
        },
        {
          scan_id: 'batch-fallback',
          report_id: 'report-fallback',
          req_id: 0,
          detection_results: [
            { detection_service: 'source_code', verdict: 'malicious', action: 'block' },
          ],
        },
      ]);

      const results = await service.pollBatch(
        {
          scanId: 'batch-fallback',
          reportId: 'report-fallback',
          entries: [
            { scanId: 'batch-fallback', reqId: 0, index: 0, prompt: 'blocked prompt' },
            { scanId: 'batch-fallback', reqId: 1, index: 1, prompt: 'allowed prompt' },
          ],
        },
        0,
      );

      expect(mockScannerInstance.queryByReportIds).toHaveBeenCalledWith(['report-fallback'], {
        numRetries: 0,
      });
      expect(results).toHaveLength(2);
      expect(results[0]).toMatchObject({
        index: 0,
        prompt: 'blocked prompt',
        action: 'block',
        triggered: true,
        detections: { source_code: true },
      });
      expect(results[1]).toMatchObject({
        index: 1,
        prompt: 'allowed prompt',
        action: 'allow',
        triggered: false,
        detections: { injection: false },
      });
    });

    it('fails closed when AIRS returns an unknown action', async () => {
      mockScannerInstance.queryByScanIds.mockResolvedValueOnce([
        {
          scan_id: 'scan-unknown-action',
          req_id: 0,
          status: 'complete',
          result: {
            scan_id: 'scan-unknown-action',
            report_id: 'report-unknown-action',
            action: 'unexpected-new-action',
            category: 'unknown',
            timeout: false,
            error: false,
            errors: [],
          },
        },
      ]);

      const [result] = await service.pollBatch(
        {
          scanId: 'scan-unknown-action',
          entries: [{ scanId: 'scan-unknown-action', reqId: 0, index: 0, prompt: 'prompt' }],
        },
        0,
      );

      expect(result.action).toBe('failed');
      expect(result.category).toBe('error');
      expect(result.error).toMatch(/unknown AIRS action/i);
    });

    it('does not ignore a blocking detector introduced by AIRS', async () => {
      mockScannerInstance.queryByScanIds.mockResolvedValueOnce([
        {
          scan_id: 'batch-future-detector',
          status: 'complete',
          result: { scan_id: 'batch-future-detector', report_id: 'report-future-detector' },
        },
      ]);
      mockScannerInstance.queryByReportIds.mockResolvedValueOnce([
        {
          scan_id: 'batch-future-detector',
          report_id: 'report-future-detector',
          req_id: 0,
          detection_results: [
            { detection_service: 'future_detector', verdict: 'malicious', action: 'block' },
          ],
        },
      ]);

      const [result] = await service.pollBatch(
        {
          scanId: 'batch-future-detector',
          reportId: 'report-future-detector',
          entries: [{ scanId: 'batch-future-detector', reqId: 0, index: 0, prompt: 'prompt' }],
        },
        0,
      );

      expect(result).toMatchObject({
        action: 'block',
        category: 'malicious',
        triggered: true,
        detections: { future_detector: true },
      });
    });

    it('does not ignore a blocking report row whose detector name is missing', async () => {
      mockScannerInstance.queryByScanIds.mockResolvedValueOnce([
        {
          scan_id: 'batch-unnamed-detector',
          status: 'complete',
          result: { scan_id: 'batch-unnamed-detector', report_id: 'report-unnamed-detector' },
        },
      ]);
      mockScannerInstance.queryByReportIds.mockResolvedValueOnce([
        {
          scan_id: 'batch-unnamed-detector',
          report_id: 'report-unnamed-detector',
          req_id: 0,
          detection_results: [{ verdict: 'malicious', action: 'block' }],
        },
      ]);

      const [result] = await service.pollBatch(
        {
          scanId: 'batch-unnamed-detector',
          reportId: 'report-unnamed-detector',
          entries: [{ scanId: 'batch-unnamed-detector', reqId: 0, index: 0, prompt: 'prompt' }],
        },
        0,
      );

      expect(result).toMatchObject({
        action: 'block',
        category: 'malicious',
        triggered: true,
        detections: { unknown: true },
      });
    });

    it('fails every unresolved prompt when AIRS reports a batch-level failure', async () => {
      mockScannerInstance.queryByScanIds.mockResolvedValueOnce([
        { scan_id: 'batch-level-failure', status: 'failed' },
      ]);

      const results = await service.pollBatch(
        {
          scanId: 'batch-level-failure',
          entries: [
            { scanId: 'batch-level-failure', reqId: 0, index: 0, prompt: 'first' },
            { scanId: 'batch-level-failure', reqId: 1, index: 1, prompt: 'second' },
          ],
        },
        0,
      );

      expect(results.map((result) => result.action)).toEqual(['failed', 'failed']);
      expect(results.map((result) => result.prompt)).toEqual(['first', 'second']);
      expect(mockScannerInstance.queryByReportIds).not.toHaveBeenCalled();
    });

    it('rejects a nested result whose scan ID conflicts with the correlated row', async () => {
      mockScannerInstance.queryByScanIds.mockResolvedValueOnce([
        {
          scan_id: 'outer-scan',
          req_id: 0,
          status: 'complete',
          result: {
            scan_id: 'different-inner-scan',
            report_id: 'report-mismatch',
            action: 'allow',
            category: 'benign',
          },
        },
      ]);

      await expect(
        service.pollBatch(
          {
            scanId: 'outer-scan',
            entries: [{ scanId: 'outer-scan', reqId: 0, index: 0, prompt: 'prompt' }],
          },
          0,
        ),
      ).rejects.toThrow(/correlation.*scan id/i);
    });

    it('reports failed and timed-out prompts as failed instead of allowed results', async () => {
      mockScannerInstance.queryByScanIds.mockResolvedValueOnce([
        { scan_id: 'batch-errors', req_id: 0, status: 'failed' },
        {
          scan_id: 'batch-errors',
          req_id: 1,
          status: 'complete',
          result: {
            scan_id: 'batch-errors',
            report_id: 'report-errors',
            action: 'allow',
            category: 'benign',
            timeout: true,
            error: true,
            errors: [{ feature: 'source_code', status: 'timeout' }],
          },
        },
      ]);

      const results = await service.pollBatch(
        {
          scanId: 'batch-errors',
          entries: [
            { scanId: 'batch-errors', reqId: 0, index: 0, prompt: 'failed prompt' },
            { scanId: 'batch-errors', reqId: 1, index: 1, prompt: 'timed out prompt' },
          ],
        },
        0,
      );

      expect(results.map((result) => result.action)).toEqual(['failed', 'failed']);
      expect(results.map((result) => result.category)).toEqual(['error', 'error']);
      expect(results[1].error).toContain('source_code');
      expect(results[1].error).toContain('timeout');
    });

    it('stops after a bounded number of polls without progress', async () => {
      mockScannerInstance.queryByScanIds
        .mockResolvedValueOnce([{ scan_id: 'stalled', req_id: 0, status: 'pending' }])
        .mockResolvedValueOnce([{ scan_id: 'stalled', req_id: 0, status: 'pending' }])
        .mockRejectedValueOnce(new Error('polling was not bounded'));

      await expect(
        service.pollBatch(
          {
            scanId: 'stalled',
            entries: [{ scanId: 'stalled', reqId: 0, index: 0, prompt: 'waiting' }],
          },
          0,
          { maxNoProgressPolls: 2 },
        ),
      ).rejects.toThrow(/no progress after 2 polls/i);
      expect(mockScannerInstance.queryByScanIds).toHaveBeenCalledTimes(2);
    });

    it('uses Retry-After when a polling GET receives 429', async () => {
      const rateLimit = Object.assign(new Error('rate limited'), {
        failureKind: 'http',
        statusCode: 429,
        retryAfterMs: 0,
      });
      const onRetry = vi.fn();
      mockScannerInstance.queryByScanIds.mockRejectedValueOnce(rateLimit).mockResolvedValueOnce([
        {
          scan_id: 'poll-rate-limit',
          req_id: 0,
          status: 'complete',
          result: {
            scan_id: 'poll-rate-limit',
            report_id: 'report-poll',
            action: 'allow',
            category: 'benign',
          },
        },
      ]);

      await service.pollBatch(
        {
          scanId: 'poll-rate-limit',
          entries: [{ scanId: 'poll-rate-limit', reqId: 0, index: 0, prompt: 'prompt' }],
        },
        0,
        { baseDelayMs: 1, onRetry },
      );

      expect(onRetry).toHaveBeenCalledWith(1, 0);
      expect(mockScannerInstance.queryByScanIds.mock.calls).toEqual([
        [['poll-rate-limit'], { numRetries: 0 }],
        [['poll-rate-limit'], { numRetries: 0 }],
      ]);
    });
  });

  describe('pollResults', () => {
    it('preserves detections on the deprecated compatibility path', async () => {
      mockScannerInstance.queryByScanIds.mockResolvedValueOnce([
        {
          scan_id: 's-detection',
          status: 'complete',
          result: {
            scan_id: 's-detection',
            report_id: 'r-detection',
            action: 'block',
            category: 'suspicious',
            prompt_detected: { source_code: true },
          },
        },
      ]);

      const [result] = await service.pollResults(['s-detection'], 0);

      expect(result.action).toBe('block');
      expect(result.triggered).toBe(true);
      expect(result.detections).toEqual({ source_code: true });
    });

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
      expect(results[1].action).toBe('allow');
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

    it('6 prompts creates 1 batch', async () => {
      mockScannerInstance.asyncScan.mockResolvedValueOnce({
        received: '2026-03-09T00:00:00Z',
        scan_id: 'batch-a',
      });

      const prompts = Array.from({ length: 6 }, (_, i) => `p${i}`);
      const scanIds = await service.submitBulkScan('profile', prompts);
      expect(mockScannerInstance.asyncScan).toHaveBeenCalledOnce();
      expect(scanIds).toEqual(['batch-a']);
      expect(mockScannerInstance.asyncScan.mock.calls[0][0]).toHaveLength(6);
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
    it('preserves operational failure details in the CSV', () => {
      const csv = SdkRuntimeService.formatResultsCsv([
        {
          prompt: 'timed out prompt',
          scanId: 's-error',
          reportId: '',
          action: 'failed',
          category: 'error',
          triggered: false,
          detections: {},
          error: 'source_code: timeout',
        },
      ]);

      const [header, row] = csv.split('\n');
      expect(header.endsWith(',error')).toBe(true);
      expect(row.endsWith('"source_code: timeout"')).toBe(true);
    });

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
      expect(lines[0]).toBe(
        'prompt,action,category,triggered,topic_violation,injection,toxic_content,dlp,url_cats,malicious_code,source_code,agent,scan_id,report_id,error',
      );
      expect(lines[1]).toBe(
        '"hello","allow","benign","false","false","false","false","false","false","false","false","false","s1","r1",""',
      );
      expect(lines[2]).toBe(
        '"hack it","block","malicious","true","false","true","false","false","false","false","false","false","s2","r2",""',
      );
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
      expect(lines[1]).toBe(
        '"hello, world, test","allow","benign","false","false","false","false","false","false","false","false","false","s1","r1",""',
      );
    });

    it('returns header only for empty results array', () => {
      const csv = SdkRuntimeService.formatResultsCsv([]);
      expect(csv).toBe(
        'prompt,action,category,triggered,topic_violation,injection,toxic_content,dlp,url_cats,malicious_code,source_code,agent,scan_id,report_id,error',
      );
    });
  });
});
