import { mkdtempSync, readFileSync, rmSync, utimesSync, writeFileSync } from 'node:fs';
import { readdirSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  installDebugLogger,
  isAirsUrl,
  pruneDebugLogs,
  redactDeep,
  redactHeaders,
  redactUrl,
} from '../../../src/cli/debug-logger.js';

describe('isAirsUrl', () => {
  it('matches each AIRS production host', () => {
    expect(isAirsUrl('https://api.sase.paloaltonetworks.com/foo')).toBe(true);
    expect(isAirsUrl('https://service.api.aisecurity.paloaltonetworks.com/foo')).toBe(true);
    expect(isAirsUrl('https://auth.apps.paloaltonetworks.com/oauth2/access_token')).toBe(true);
    expect(isAirsUrl('https://api.dlp.paloaltonetworks.com/v2/api/data-filtering-profiles')).toBe(
      true,
    );
  });

  it('matches regional subdomains via endsWith', () => {
    expect(isAirsUrl('https://us.api.sase.paloaltonetworks.com/foo')).toBe(true);
    expect(isAirsUrl('https://eu.api.dlp.paloaltonetworks.com/foo')).toBe(true);
  });

  it('rejects non-AIRS hosts and malformed URLs', () => {
    expect(isAirsUrl('https://example.com/foo')).toBe(false);
    expect(isAirsUrl('https://paloaltonetworks.com/foo')).toBe(false);
    expect(isAirsUrl('not-a-url')).toBe(false);
  });
});

describe('redactHeaders', () => {
  it('fully masks sensitive headers with no value prefix retained', () => {
    const out = redactHeaders({
      Authorization: 'Bearer abcdefghijklmnop',
      'x-pan-token': 'tok-1234567890abcdef',
      Cookie: 'session=secret',
      'X-Api-Key': 'key-abc',
    });
    expect(out.Authorization).toBe('***');
    expect(out['x-pan-token']).toBe('***');
    expect(out.Cookie).toBe('***');
    expect(out['X-Api-Key']).toBe('***');
    expect(JSON.stringify(out)).not.toContain('abcdefgh');
  });

  it('leaves non-sensitive headers untouched', () => {
    const out = redactHeaders({ 'content-type': 'application/json', accept: '*/*' });
    expect(out['content-type']).toBe('application/json');
    expect(out.accept).toBe('*/*');
  });
});

describe('redactDeep', () => {
  it('masks sensitive keys recursively in nested objects and arrays', () => {
    const input = {
      profile_name: 'demo',
      api_key: 'sk-live-12345',
      nested: { client_secret: 'oauth-secret', ok: 'visible' },
      list: [{ password: 'hunter2' }, { token: 'tok' }, 'plain'],
    };
    const out = redactDeep(input) as Record<string, unknown>;
    expect(out.profile_name).toBe('demo');
    expect(out.api_key).toBe('***');
    expect((out.nested as Record<string, unknown>).client_secret).toBe('***');
    expect((out.nested as Record<string, unknown>).ok).toBe('visible');
    const list = out.list as Array<Record<string, unknown> | string>;
    expect((list[0] as Record<string, unknown>).password).toBe('***');
    expect((list[1] as Record<string, unknown>).token).toBe('***');
    expect(list[2]).toBe('plain');
  });

  it('passes through primitives, null, and undefined', () => {
    expect(redactDeep('text')).toBe('text');
    expect(redactDeep(42)).toBe(42);
    expect(redactDeep(null)).toBe(null);
    expect(redactDeep(undefined)).toBe(undefined);
  });

  it('does not mutate the input object', () => {
    const input = { token: 'orig' };
    redactDeep(input);
    expect(input.token).toBe('orig');
  });
});

describe('redactUrl', () => {
  it('masks sensitive query params and preserves others', () => {
    const out = redactUrl('https://api.example.com/v1?api_key=sk-123&page=2&access_token=tok');
    expect(out).toContain('api_key=***');
    expect(out).toContain('access_token=***');
    expect(out).toContain('page=2');
    expect(out).not.toContain('sk-123');
  });

  it('returns malformed URLs unchanged', () => {
    expect(redactUrl('not-a-url')).toBe('not-a-url');
  });
});

describe('pruneDebugLogs', () => {
  it('keeps only the newest N debug-api files and ignores other files', () => {
    const dir = mkdtempSync(join(tmpdir(), 'airs-debug-'));
    try {
      const base = Date.now() / 1000 - 1000;
      for (let i = 0; i < 13; i++) {
        const p = join(dir, `debug-api-${1000 + i}.jsonl`);
        writeFileSync(p, '{}\n');
        utimesSync(p, base + i, base + i);
      }
      writeFileSync(join(dir, 'config.json'), '{}');
      pruneDebugLogs(dir, 10);
      const remaining = readdirSync(dir).filter((f) => f.startsWith('debug-api-'));
      expect(remaining).toHaveLength(10);
      expect(remaining).not.toContain('debug-api-1000.jsonl');
      expect(remaining).not.toContain('debug-api-1002.jsonl');
      expect(remaining).toContain('debug-api-1012.jsonl');
      expect(readdirSync(dir)).toContain('config.json');
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });

  it('tolerates a missing directory', () => {
    expect(() => pruneDebugLogs('/nonexistent/airs-debug-dir', 10)).not.toThrow();
  });
});

describe('installDebugLogger redaction integration', () => {
  const originalFetch = globalThis.fetch;

  afterEach(() => {
    globalThis.fetch = originalFetch;
  });

  it('writes redacted request/response bodies, headers, and URLs to the log', async () => {
    const dir = mkdtempSync(join(tmpdir(), 'airs-debug-'));
    const logPath = join(dir, 'debug-api-test.jsonl');
    try {
      globalThis.fetch = vi.fn(async () =>
        Response.json({ result: 'ok', session_token: 'resp-secret' }),
      ) as unknown as typeof fetch;

      const { teardown } = installDebugLogger(logPath);
      await fetch('https://api.sase.paloaltonetworks.com/v1/thing?api_key=sk-leak&x=1', {
        method: 'POST',
        headers: { authorization: 'Bearer leak-me', 'content-type': 'application/json' },
        body: JSON.stringify({ prompt: 'hello', client_secret: 'body-leak' }),
      });
      teardown();

      const raw = readFileSync(logPath, 'utf-8');
      expect(raw).not.toContain('leak-me');
      expect(raw).not.toContain('sk-leak');
      expect(raw).not.toContain('body-leak');
      expect(raw).not.toContain('resp-secret');
      const entry = JSON.parse(raw.trim());
      expect(entry.request.url).toContain('api_key=***');
      expect(entry.request.headers.authorization).toBe('***');
      expect(entry.request.body.client_secret).toBe('***');
      expect(entry.request.body.prompt).toBe('hello');
      expect(entry.response.body.session_token).toBe('***');
      expect(entry.response.body.result).toBe('ok');
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });
});
