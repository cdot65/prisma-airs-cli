import {
  appendFileSync,
  mkdirSync,
  readdirSync,
  statSync,
  unlinkSync,
  writeFileSync,
} from 'node:fs';
import { dirname, join } from 'node:path';

/** Domains that indicate AIRS / Strata Cloud Manager API traffic. */
const AIRS_DOMAINS = [
  'api.sase.paloaltonetworks.com',
  'api.apps.paloaltonetworks.com',
  'service.api.aisecurity.paloaltonetworks.com',
  'auth.apps.paloaltonetworks.com',
  'api.dlp.paloaltonetworks.com',
];

export function isAirsUrl(url: string): boolean {
  try {
    const parsed = new URL(url);
    return AIRS_DOMAINS.some((d) => parsed.hostname === d || parsed.hostname.endsWith(`.${d}`));
  } catch {
    return false;
  }
}

const MASK = '***';

/** Key names (headers, query params, JSON body fields) whose values are secrets. */
const SENSITIVE_KEY_PATTERN =
  /token|secret|password|passwd|credential|authorization|cookie|api[-_]?key|client[-_]?auth|auth[-_]?code|^key$/i;

function isSensitiveKey(key: string): boolean {
  return SENSITIVE_KEY_PATTERN.test(key);
}

/** Fully mask sensitive header values — no prefix or suffix retained. */
export function redactHeaders(headers: Record<string, string>): Record<string, string> {
  const out: Record<string, string> = {};
  for (const [k, v] of Object.entries(headers)) {
    out[k] =
      isSensitiveKey(k) || /^x-portkey-(config|metadata|forward-headers)$/i.test(k) ? MASK : v;
  }
  return out;
}

/** Recursively mask values of sensitive keys in parsed JSON structures. Non-mutating. */
export function redactDeep(value: unknown): unknown {
  if (Array.isArray(value)) {
    return value.map(redactDeep);
  }
  if (value !== null && typeof value === 'object') {
    const out: Record<string, unknown> = {};
    for (const [k, v] of Object.entries(value as Record<string, unknown>)) {
      out[k] = isSensitiveKey(k) ? MASK : redactDeep(v);
    }
    return out;
  }
  return value;
}

/** Mask sensitive query-parameter values; malformed URLs pass through unchanged. */
export function redactUrl(url: string): string {
  try {
    const parsed = new URL(url);
    let touched = false;
    for (const key of parsed.searchParams.keys()) {
      if (isSensitiveKey(key)) {
        parsed.searchParams.set(key, MASK);
        touched = true;
      }
    }
    return touched ? parsed.toString() : url;
  } catch {
    return url;
  }
}

/** Delete all but the newest `keep` debug-api-*.jsonl files in `dir`. */
export function pruneDebugLogs(dir: string, keep: number): void {
  let files: string[];
  try {
    files = readdirSync(dir).filter((f) => f.startsWith('debug-api-') && f.endsWith('.jsonl'));
  } catch {
    return;
  }
  const byAge = files
    .map((f) => {
      const path = join(dir, f);
      try {
        return { path, mtime: statSync(path).mtimeMs };
      } catch {
        return null;
      }
    })
    .filter((e): e is { path: string; mtime: number } => e !== null)
    .sort((a, b) => b.mtime - a.mtime);
  for (const { path } of byAge.slice(keep)) {
    try {
      unlinkSync(path);
    } catch {
      // best-effort cleanup; a locked or already-deleted file must not break the CLI
    }
  }
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
  mkdirSync(dirname(logPath), { recursive: true, mode: 0o700 });
  writeFileSync(logPath, '', { encoding: 'utf8', mode: 0o600, flag: 'wx' });
  // CWD artifacts belong to the user: never truncate or automatically prune earlier logs.

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

    const rawHeaders = headersToRecord(
      init?.headers ?? (input instanceof Request ? input.headers : undefined),
    );
    const inference = Object.keys(rawHeaders).some(
      (name) => name.toLowerCase() === 'x-portkey-api-key',
    );
    if (!isAirsUrl(url) && !inference) {
      return originalFetch(input, init);
    }
    const dashboard = /\/v1\/mgmt\/(dashboard\/|reports\/scancontent(?:\?|$))/.test(url);

    const method = init?.method ?? (input instanceof Request ? input.method : 'GET');
    const reqHeaders = redactHeaders(rawHeaders);
    const loggedUrl = redactUrl(url);

    let reqBody: unknown;
    if (inference || dashboard) reqBody = '[BODY OMITTED]';
    else if (init?.body) {
      try {
        reqBody = redactDeep(JSON.parse(String(init.body)));
      } catch {
        const contentType = new Headers(rawHeaders).get('content-type') ?? '';
        reqBody = contentType.includes('application/x-www-form-urlencoded')
          ? redactDeep(Object.fromEntries(new URLSearchParams(String(init.body))))
          : '[NON-JSON BODY OMITTED]';
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
      error = inference ? 'Runtime request failed' : 'Request failed';
      const entry = JSON.stringify({
        timestamp: ts,
        durationMs: Date.now() - startMs,
        request: { method, url: loggedUrl, headers: reqHeaders, body: reqBody },
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
    if (
      inference ||
      dashboard ||
      response.headers.get('content-type')?.includes('text/event-stream')
    ) {
      // Never consume/tee an inference stream before the SDK can read it. Besides retaining
      // sensitive prompt text, clone().text() would buffer the full stream and defeat cancellation.
      resBody = '[BODY OMITTED]';
    } else {
      const clone = response.clone();
      try {
        const text = await clone.text();
        try {
          resBody = redactDeep(JSON.parse(text));
        } catch {
          resBody = '[NON-JSON BODY OMITTED]';
        }
      } catch {
        resBody = '<unreadable>';
      }
    }

    const entry = JSON.stringify({
      timestamp: ts,
      durationMs,
      request: { method, url: loggedUrl, headers: reqHeaders, body: reqBody },
      response: {
        status: response.status,
        statusText: response.statusText,
        headers: redactHeaders(resHeaders),
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
