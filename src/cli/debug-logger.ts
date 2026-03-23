import { appendFileSync, mkdirSync, writeFileSync } from 'node:fs';
import { dirname } from 'node:path';

/** Domains that indicate AIRS / Strata Cloud Manager API traffic. */
const AIRS_DOMAINS = [
  'api.sase.paloaltonetworks.com',
  'service.api.aisecurity.paloaltonetworks.com',
  'auth.apps.paloaltonetworks.com',
];

function isAirsUrl(url: string): boolean {
  try {
    const parsed = new URL(url);
    return AIRS_DOMAINS.some((d) => parsed.hostname === d || parsed.hostname.endsWith(`.${d}`));
  } catch {
    return false;
  }
}

function redactAuth(headers: Record<string, string>): Record<string, string> {
  const out: Record<string, string> = {};
  for (const [k, v] of Object.entries(headers)) {
    const lower = k.toLowerCase();
    if (lower === 'authorization' || lower === 'x-pan-token') {
      out[k] = v.length > 12 ? `${v.slice(0, 8)}...${v.slice(-4)}` : '***';
    } else {
      out[k] = v;
    }
  }
  return out;
}

function headersToRecord(
  headers: NonNullable<RequestInit['headers']> | undefined,
): Record<string, string> {
  if (!headers) return {};
  if (
    typeof headers === 'object' &&
    'forEach' in headers &&
    typeof headers.forEach === 'function'
  ) {
    const out: Record<string, string> = {};
    (headers as Headers).forEach((v, k) => {
      out[k] = v;
    });
    return out;
  }
  if (Array.isArray(headers)) {
    return Object.fromEntries(headers);
  }
  return headers as Record<string, string>;
}

/**
 * Install a global fetch interceptor that logs all AIRS / SCM API
 * requests and responses to a JSONL file.
 *
 * Returns the log file path and a teardown function.
 */
export function installDebugLogger(logPath: string): { teardown: () => void } {
  mkdirSync(dirname(logPath), { recursive: true });
  writeFileSync(logPath, '', 'utf-8'); // truncate / create

  const originalFetch = globalThis.fetch;

  globalThis.fetch = async function debugFetch(
    input: string | URL | Request,
    init?: RequestInit,
  ): Promise<Response> {
    const url =
      typeof input === 'string'
        ? input
        : input instanceof URL
          ? input.toString()
          : (input as Request).url;

    if (!isAirsUrl(url)) {
      return originalFetch(input, init);
    }

    const method = init?.method ?? (input instanceof Request ? input.method : 'GET');
    const reqHeaders = redactAuth(headersToRecord(init?.headers));

    let reqBody: unknown;
    if (init?.body) {
      try {
        reqBody = JSON.parse(String(init.body));
      } catch {
        reqBody = String(init.body);
      }
    }

    const ts = new Date().toISOString();
    const startMs = Date.now();

    let response: Response;
    let resBody: unknown;
    let error: string | undefined;

    try {
      response = await originalFetch(input, init);
    } catch (err) {
      error = err instanceof Error ? err.message : String(err);
      const entry = JSON.stringify({
        timestamp: ts,
        durationMs: Date.now() - startMs,
        request: { method, url, headers: reqHeaders, body: reqBody },
        error,
      });
      appendFileSync(logPath, `${entry}\n`);
      throw err;
    }

    const durationMs = Date.now() - startMs;
    const resHeaders: Record<string, string> = {};
    response.headers.forEach((v, k) => {
      resHeaders[k] = v;
    });

    // Clone so the original consumer can still read the body
    const clone = response.clone();
    try {
      const text = await clone.text();
      try {
        resBody = JSON.parse(text);
      } catch {
        resBody = text;
      }
    } catch {
      resBody = '<unreadable>';
    }

    const entry = JSON.stringify({
      timestamp: ts,
      durationMs,
      request: { method, url, headers: reqHeaders, body: reqBody },
      response: {
        status: response.status,
        statusText: response.statusText,
        headers: resHeaders,
        body: resBody,
      },
    });
    appendFileSync(logPath, `${entry}\n`);

    return response;
  };

  return {
    teardown() {
      globalThis.fetch = originalFetch;
    },
  };
}
