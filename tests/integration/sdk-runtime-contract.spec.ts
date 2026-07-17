import { init, MAX_NUMBER_OF_BATCH_SCAN_OBJECTS, Scanner } from '@cdot65/prisma-airs-sdk';
import { afterEach, describe, expect, it, vi } from 'vitest';

const REQUEST = [
  {
    req_id: 0,
    scan_req: {
      ai_profile: { profile_name: 'contract-profile' },
      contents: [{ prompt: 'contract prompt' }],
    },
  },
];

describe('published SDK runtime contract', () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it('makes exactly one async POST attempt when per-call retries are disabled', async () => {
    const fetchMock = vi.fn().mockRejectedValue(new TypeError('connection reset'));
    vi.stubGlobal('fetch', fetchMock);
    init({
      apiKey: 'contract-api-key',
      apiEndpoint: 'https://contract.invalid',
      numRetries: 5,
    });

    await expect(new Scanner().asyncScan(REQUEST, { numRetries: 0 })).rejects.toMatchObject({
      failureKind: 'network',
    });
    expect(fetchMock).toHaveBeenCalledOnce();
  });

  it('exposes confirmed HTTP 429 status and normalized Retry-After metadata', async () => {
    const fetchMock = vi.fn().mockResolvedValue(
      new Response(JSON.stringify({ message: 'rate limited' }), {
        status: 429,
        headers: { 'Retry-After': '2' },
      }),
    );
    vi.stubGlobal('fetch', fetchMock);
    init({
      apiKey: 'contract-api-key',
      apiEndpoint: 'https://contract.invalid',
      numRetries: 5,
    });

    await expect(new Scanner().asyncScan(REQUEST, { numRetries: 0 })).rejects.toMatchObject({
      failureKind: 'http',
      statusCode: 429,
      retryAfterMs: 2000,
    });
    expect(fetchMock).toHaveBeenCalledOnce();
  });

  it('accepts the SDK 0.13.2 maximum of 20 async request objects', async () => {
    const fetchMock = vi
      .fn()
      .mockResolvedValue(
        new Response(
          JSON.stringify({ received: '2026-07-17T00:00:00Z', scan_id: 'contract-batch-20' }),
          { status: 200 },
        ),
      );
    vi.stubGlobal('fetch', fetchMock);
    init({ apiKey: 'contract-api-key', apiEndpoint: 'https://contract.invalid' });
    const requests = Array.from({ length: 20 }, (_, reqId) => ({
      req_id: reqId,
      scan_req: {
        ai_profile: { profile_name: 'contract-profile' },
        contents: [{ prompt: `contract prompt ${reqId}` }],
      },
    }));

    expect(MAX_NUMBER_OF_BATCH_SCAN_OBJECTS).toBe(20);
    await expect(new Scanner().asyncScan(requests, { numRetries: 0 })).resolves.toMatchObject({
      scan_id: 'contract-batch-20',
    });
    expect(fetchMock).toHaveBeenCalledOnce();
  });
});
