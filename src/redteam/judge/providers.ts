import { createHash } from 'node:crypto';
import type { JudgeState } from './ingest.js';
import {
  DEFAULT_TYPESAFE_BASE_URL,
  DEFAULT_TYPESAFE_MODEL,
  type JudgeQuestion,
  SCHEMA_VERSION,
} from './questions.js';

/** Raised when a provider cannot produce answers for a unit. */
export class ProviderError extends Error {
  constructor(message: string) {
    super(message);
    this.name = 'ProviderError';
  }
}

/** Provider output in the TypeSafe wire shape, plus request metadata. */
export interface RawJudgment {
  answers: Record<string, Record<string, unknown>>;
  model: string;
  usage: Record<string, unknown>;
  request_id: string | null;
  latency_ms: number | null;
}

export interface JudgeRequest {
  unitId: string;
  state: JudgeState;
  questions: Readonly<Record<string, JudgeQuestion>>;
}

export interface JudgeProvider {
  readonly name: string;
  readonly model: string;
  judge(request: JudgeRequest): Promise<RawJudgment>;
}

/** One recorded unit inside a `--record` / `--replay` file. */
export interface RecordedJudgment {
  answers: Record<string, Record<string, unknown>>;
  model: string;
  usage: Record<string, unknown>;
  request_id: string | null;
  prompt_sha256?: string;
  response_sha256?: string;
}

export interface RecordingFile {
  schema_version: number;
  model: string | null;
  questions: Readonly<Record<string, JudgeQuestion>>;
  judgments: Record<string, RecordedJudgment>;
}

type FetchLike = typeof globalThis.fetch;
type Sleep = (ms: number) => Promise<void>;

const defaultSleep: Sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

/**
 * Backoff per the documented guidance (retry 429/529 with exponential backoff instead of
 * immediately): honor `retry-after-ms` / `Retry-After` when the response carries one,
 * else 0.5s doubling per attempt, capped at 5s, plus up to 25% jitter.
 */
export function retryDelayMs(
  attempt: number,
  headers: Headers | null,
  random: () => number = Math.random,
): number {
  if (headers) {
    for (const [header, multiplier] of [
      ['retry-after-ms', 1],
      ['retry-after', 1000],
    ] as const) {
      const value = headers.get(header);
      if (value && Number.isFinite(Number(value)) && Number(value) >= 0)
        return Math.min(Number(value) * multiplier, 60_000);
    }
  }
  const base = Math.min(500 * 2 ** attempt, 5000);
  return base + base * 0.25 * random();
}

/** Refuse cleartext remote endpoints and URL credentials before sending a key. */
export function typesafeEndpoint(baseUrl: string, path: string): string {
  let url: URL;
  try {
    url = new URL(baseUrl);
  } catch {
    throw new ProviderError('Invalid TypeSafe base URL');
  }
  const loopback = ['localhost', '127.0.0.1', '[::1]'].includes(url.hostname);
  if (
    (url.protocol !== 'https:' && !(url.protocol === 'http:' && loopback)) ||
    url.username ||
    url.password ||
    url.search ||
    url.hash
  )
    throw new ProviderError(
      'TypeSafe base URL requires HTTPS (HTTP only for loopback) without credentials, query or fragment',
    );
  return `${url.toString().replace(/\/+$/, '')}${path}`;
}

export interface TypeSafeHttpProviderOptions {
  apiKey: string;
  model?: string;
  baseUrl?: string;
  maxRetries?: number;
  timeoutMs?: number;
  fetch?: FetchLike;
  sleep?: Sleep;
}

/**
 * Client for `POST {baseUrl}/v1/systemone` per https://docs.typesafe.ai/api.md. Retries
 * 408, 429 and 5xx (including 529 overloaded) with backoff; never retries 401/422.
 * Response bodies are parsed only for the documented `answers`/`model`/`usage` keys and are
 * never included in error messages.
 */
export class TypeSafeHttpProvider implements JudgeProvider {
  readonly name = 'typesafe-http';
  readonly model: string;
  private readonly url: string;
  private readonly apiKey: string;
  private readonly maxRetries: number;
  private readonly timeoutMs: number;
  private readonly fetch: FetchLike;
  private readonly sleep: Sleep;

  constructor(options: TypeSafeHttpProviderOptions) {
    this.apiKey = options.apiKey;
    this.model = options.model ?? DEFAULT_TYPESAFE_MODEL;
    this.url = typesafeEndpoint(options.baseUrl ?? DEFAULT_TYPESAFE_BASE_URL, '/v1/systemone');
    this.maxRetries = options.maxRetries ?? 2;
    this.timeoutMs = options.timeoutMs ?? 30_000;
    this.fetch = options.fetch ?? globalThis.fetch;
    this.sleep = options.sleep ?? defaultSleep;
  }

  async judge(request: JudgeRequest): Promise<RawJudgment> {
    const body = JSON.stringify({
      state: request.state,
      model: this.model,
      questions: request.questions,
    });
    for (let attempt = 0; ; attempt += 1) {
      const started = performance.now();
      let response: Response;
      try {
        response = await this.fetch(this.url, {
          method: 'POST',
          redirect: 'error',
          headers: {
            Authorization: `Bearer ${this.apiKey}`,
            'Content-Type': 'application/json',
            Accept: 'application/json',
          },
          body,
          signal: AbortSignal.timeout(this.timeoutMs),
        });
      } catch (error) {
        if (attempt >= this.maxRetries)
          throw new ProviderError(
            `TypeSafe API connection failed: ${error instanceof Error ? error.name : 'Error'}`,
          );
        await this.sleep(retryDelayMs(attempt, null));
        continue;
      }
      if (response.ok) {
        let decoded: unknown;
        try {
          decoded = await response.json();
        } catch {
          throw new ProviderError('TypeSafe API returned invalid JSON');
        }
        if (decoded === null || typeof decoded !== 'object' || Array.isArray(decoded))
          throw new ProviderError('TypeSafe API response must be an object');
        const payload = decoded as Record<string, unknown>;
        const answers = payload.answers;
        if (answers === null || typeof answers !== 'object' || Array.isArray(answers))
          throw new ProviderError('TypeSafe API response has no answers map');
        return {
          answers: answers as Record<string, Record<string, unknown>>,
          model: typeof payload.model === 'string' ? payload.model : this.model,
          usage:
            payload.usage !== null && typeof payload.usage === 'object'
              ? (payload.usage as Record<string, unknown>)
              : {},
          request_id: response.headers.get('x-typesafe-request-id'),
          latency_ms: performance.now() - started,
        };
      }
      const status = response.status;
      await response.body?.cancel().catch(() => undefined);
      const retryable = status === 408 || status === 429 || (status >= 500 && status <= 599);
      if (!retryable || attempt >= this.maxRetries)
        throw new ProviderError(`TypeSafe API returned HTTP ${status}`);
      await this.sleep(retryDelayMs(attempt, response.headers));
    }
  }
}

/** Serves recorded raw judgments keyed by unit id; used for tests and reproducibility. */
export class ReplayProvider implements JudgeProvider {
  readonly name = 'replay';
  readonly model: string;
  private readonly recorded: Record<string, Partial<RecordedJudgment>>;

  constructor(recorded: Record<string, unknown>) {
    const judgments = recorded.judgments;
    this.recorded = (
      judgments !== null && typeof judgments === 'object' ? judgments : recorded
    ) as Record<string, Partial<RecordedJudgment>>;
    this.model = String(recorded.model ?? 'replay');
  }

  async judge(request: JudgeRequest): Promise<RawJudgment> {
    const entry = this.recorded[request.unitId];
    if (!entry || typeof entry !== 'object' || !entry.answers)
      throw new ProviderError(`no recorded judgment for unit ${request.unitId}`);
    for (const [field, text] of [
      ['prompt_sha256', request.state.attack.prompt],
      ['response_sha256', request.state.target_response],
    ] as const) {
      const expected = entry[field];
      const actual = createHash('sha256').update(text).digest('hex').slice(0, 16);
      if (expected !== undefined && expected !== actual)
        throw new ProviderError(`recorded ${field} does not match unit ${request.unitId}`);
    }
    return {
      answers: entry.answers,
      model: typeof entry.model === 'string' ? entry.model : this.model,
      usage: entry.usage ?? {},
      request_id: entry.request_id ?? null,
      latency_ms: null,
    };
  }
}

/** Build the `--record` file: the questions asked plus every raw answer, keyed by unit id. */
export function recordingFile(
  model: string | null,
  questions: Readonly<Record<string, JudgeQuestion>>,
  judgments: Record<string, RecordedJudgment>,
): RecordingFile {
  return { schema_version: SCHEMA_VERSION, model, questions, judgments };
}

/**
 * `GET {baseUrl}/v1/models` (documented at https://docs.typesafe.ai/models.md) lists the
 * model names the account may send; it is the cheapest authenticated call and the doctor
 * probe. Resolves with the number of listed models; rejects with `status` on any non-2xx.
 */
export async function listTypesafeModels(options: {
  apiKey: string;
  baseUrl?: string;
  timeoutMs?: number;
  fetch?: FetchLike;
}): Promise<number> {
  const fetchImpl = options.fetch ?? globalThis.fetch;
  const url = typesafeEndpoint(options.baseUrl ?? DEFAULT_TYPESAFE_BASE_URL, '/v1/models');
  const response = await fetchImpl(url, {
    redirect: 'error',
    headers: { Authorization: `Bearer ${options.apiKey}`, Accept: 'application/json' },
    signal: AbortSignal.timeout(options.timeoutMs ?? 30_000),
  });
  if (!response.ok) {
    await response.body?.cancel().catch(() => undefined);
    throw Object.assign(new Error(`TypeSafe API returned HTTP ${response.status}`), {
      status: response.status,
    });
  }
  const payload = (await response.json()) as { models?: unknown };
  if (!payload || !Array.isArray(payload.models))
    throw new ProviderError('TypeSafe models response has no models array');
  return payload.models.length;
}
