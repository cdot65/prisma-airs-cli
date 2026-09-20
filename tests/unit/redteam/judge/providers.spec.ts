import { describe, expect, it, vi } from 'vitest';
import {
  listTypesafeModels,
  ProviderError,
  QUESTIONS,
  ReplayProvider,
  recordingFile,
  retryDelayMs,
  TypeSafeHttpProvider,
} from '../../../../src/redteam/judge/index.js';

const state = {
  attack: { category: 'SECURITY', sub_category: 'x', objective: 'o', prompt: 'p' },
  target_response: 'r',
};

function answers(pSuccess: number) {
  return {
    objective_achieved: { type: 'noul', noul: pSuccess },
    response_disposition: {
      type: 'choice',
      choice: 'full_compliance',
      confidence: 0.8,
      probabilities: { full_compliance: 0.8, full_refusal: 0.2 },
    },
    objective_fulfillment: {
      type: 'score',
      score: 2.5,
      confidence: 0.7,
      legend: { '0': 'a', '1': 'b', '2': 'c', '3': 'd' },
      probabilities: { '3': 0.5, '2': 0.5 },
    },
  };
}

function ok(body: unknown, headers: Record<string, string> = {}) {
  return new Response(JSON.stringify(body), {
    status: 200,
    headers: { 'content-type': 'application/json', ...headers },
  });
}

const noSleep = vi.fn(async () => {});

describe('TypeSafeHttpProvider', () => {
  it('sends the documented payload and parses answers, model, usage and request id', async () => {
    const fetch = vi.fn(async () =>
      ok(
        {
          model: 'jev-1.13.0',
          answers: answers(0.9),
          usage: { input_tokens: 12, output_tokens: 0 },
        },
        { 'x-typesafe-request-id': 'req-1' },
      ),
    );
    const provider = new TypeSafeHttpProvider({
      apiKey: 'k',
      model: 'jev-latest',
      baseUrl: 'https://example.test/',
      fetch,
      sleep: noSleep,
    });
    const raw = await provider.judge({ unitId: 'u', state, questions: QUESTIONS });
    const [url, init] = fetch.mock.calls[0] as unknown as [string, RequestInit];
    expect(url).toBe('https://example.test/v1/systemone');
    expect(init.method).toBe('POST');
    expect((init.headers as Record<string, string>).Authorization).toBe('Bearer k');
    expect(init.signal).toBeInstanceOf(AbortSignal);
    const body = JSON.parse(init.body as string);
    expect(Object.keys(body).sort()).toEqual(['model', 'questions', 'state']);
    expect(body.model).toBe('jev-latest');
    expect(body.state).toEqual(state);
    expect(body.questions.objective_achieved.type).toBe('noul');
    expect(body.questions.response_disposition.type).toBe('choice');
    expect(body.questions.objective_fulfillment.criteria).toHaveLength(4);
    expect([raw.model, raw.request_id, raw.usage.input_tokens]).toEqual([
      'jev-1.13.0',
      'req-1',
      12,
    ]);
    expect(raw.latency_ms).toBeGreaterThanOrEqual(0);
    expect(provider.name).toBe('typesafe-http');
  });

  it('retries retryable statuses honoring Retry-After, then fails cleanly', async () => {
    const fetch = vi.fn(
      async () => new Response('{}', { status: 429, headers: { 'retry-after': '0' } }),
    );
    const sleep = vi.fn(async () => {});
    const provider = new TypeSafeHttpProvider({ apiKey: 'k', maxRetries: 2, fetch, sleep });
    await expect(provider.judge({ unitId: 'u', state, questions: QUESTIONS })).rejects.toThrow(
      'TypeSafe API returned HTTP 429',
    );
    expect(fetch).toHaveBeenCalledTimes(3);
    expect(sleep).toHaveBeenCalledTimes(2);
    expect(sleep.mock.calls.map(([ms]) => ms)).toEqual([0, 0]);
  });

  it('honors retry-after-ms and recovers after a 529 overload', async () => {
    const fetch = vi
      .fn()
      .mockResolvedValueOnce(
        new Response('', { status: 529, headers: { 'retry-after-ms': '250' } }),
      )
      .mockResolvedValueOnce(ok({ model: 'm', answers: answers(0.1), usage: {} }));
    const sleep = vi.fn(async () => {});
    const provider = new TypeSafeHttpProvider({ apiKey: 'k', fetch, sleep });
    const raw = await provider.judge({ unitId: 'u', state, questions: QUESTIONS });
    expect(raw.answers.objective_achieved.noul).toBe(0.1);
    expect(sleep).toHaveBeenCalledExactlyOnceWith(250);
  });

  it('does not retry auth or validation errors and never echoes the body', async () => {
    for (const status of [401, 422]) {
      const fetch = vi.fn(async () => new Response('{"detail":"SECRET-DETAIL"}', { status }));
      const provider = new TypeSafeHttpProvider({ apiKey: 'k', fetch, sleep: noSleep });
      const failure = await provider
        .judge({ unitId: 'u', state, questions: QUESTIONS })
        .catch((error: unknown) => error);
      expect(failure).toBeInstanceOf(ProviderError);
      expect((failure as Error).message).toBe(`TypeSafe API returned HTTP ${status}`);
      expect((failure as Error).message).not.toContain('SECRET');
      expect(fetch).toHaveBeenCalledTimes(1);
    }
  });

  it('retries network failures with backoff and reports the error class', async () => {
    const fetch = vi.fn(async () => {
      throw new DOMException('aborted', 'TimeoutError');
    });
    const sleep = vi.fn(async () => {});
    const provider = new TypeSafeHttpProvider({ apiKey: 'k', maxRetries: 1, fetch, sleep });
    await expect(provider.judge({ unitId: 'u', state, questions: QUESTIONS })).rejects.toThrow(
      'TypeSafe API connection failed: TimeoutError',
    );
    expect(fetch).toHaveBeenCalledTimes(2);
    expect(sleep.mock.calls[0][0]).toBeGreaterThanOrEqual(500);
  });

  it('rejects a 2xx payload without an answers map', async () => {
    const fetch = vi.fn(async () => ok({ model: 'm' }));
    const provider = new TypeSafeHttpProvider({ apiKey: 'k', fetch, sleep: noSleep });
    await expect(provider.judge({ unitId: 'u', state, questions: QUESTIONS })).rejects.toThrow(
      'no answers map',
    );
  });
});

describe('retryDelayMs', () => {
  it('prefers response headers, else doubles from 0.5s with a 5s cap and jitter', () => {
    const headers = new Headers({ 'retry-after-ms': '250', 'retry-after': '9' });
    expect(retryDelayMs(0, headers)).toBe(250);
    expect(retryDelayMs(0, new Headers({ 'retry-after': '2' }))).toBe(2000);
    expect(retryDelayMs(0, new Headers({ 'retry-after': 'soon' }), () => 0)).toBe(500);
    expect(retryDelayMs(3, null, () => 0)).toBe(4000);
    expect(retryDelayMs(9, null, () => 0)).toBe(5000);
    expect(retryDelayMs(9, null, () => 1)).toBe(6250);
  });
});

describe('ReplayProvider', () => {
  it('serves recorded answers by unit id and reports missing units as provider errors', async () => {
    const provider = new ReplayProvider({
      model: 'fixture',
      judgments: { 'a#0': { answers: answers(0.7), usage: { input_tokens: 3 }, request_id: 'r' } },
    });
    expect([provider.name, provider.model]).toEqual(['replay', 'fixture']);
    const raw = await provider.judge({ unitId: 'a#0', state, questions: QUESTIONS });
    expect(raw).toMatchObject({ model: 'fixture', request_id: 'r', usage: { input_tokens: 3 } });
    await expect(provider.judge({ unitId: 'b#0', state, questions: QUESTIONS })).rejects.toThrow(
      'no recorded judgment for unit b#0',
    );
  });

  it('accepts a bare judgments map and defaults the model name', async () => {
    const provider = new ReplayProvider({ 'a#0': { answers: answers(0.2), model: 'm' } });
    expect(provider.model).toBe('replay');
    const raw = await provider.judge({ unitId: 'a#0', state, questions: QUESTIONS });
    expect(raw).toMatchObject({ model: 'm', usage: {}, request_id: null, latency_ms: null });
  });

  it('round-trips through the recording file shape', () => {
    const file = recordingFile('m', QUESTIONS, {
      'a#0': { answers: answers(0.5), model: 'm', usage: {}, request_id: null },
    });
    expect(file.schema_version).toBe(1);
    expect(Object.keys(file)).toEqual(['schema_version', 'model', 'questions', 'judgments']);
    expect(new ReplayProvider(JSON.parse(JSON.stringify(file))).model).toBe('m');
  });
});

describe('listTypesafeModels', () => {
  it('lists models with the bearer key and rejects with the status on failure', async () => {
    const fetch = vi.fn(async () =>
      ok({ models: [{ name: 'jev-latest' }, { name: 'jev-preview' }] }),
    );
    await expect(listTypesafeModels({ apiKey: 'k', fetch })).resolves.toBe(2);
    const [url, init] = fetch.mock.calls[0] as unknown as [string, RequestInit];
    expect(url).toBe('https://api.typesafe.ai/v1/models');
    expect((init.headers as Record<string, string>).Authorization).toBe('Bearer k');
    const denied = vi.fn(async () => new Response('nope', { status: 401 }));
    await expect(
      listTypesafeModels({ apiKey: 'k', baseUrl: 'https://x.test/', fetch: denied }),
    ).rejects.toMatchObject({ status: 401 });
    expect(denied.mock.calls[0][0]).toBe('https://x.test/v1/models');
    const odd = vi.fn(async () => ok({}));
    await expect(listTypesafeModels({ apiKey: 'k', fetch: odd })).resolves.toBe(0);
  });
});
