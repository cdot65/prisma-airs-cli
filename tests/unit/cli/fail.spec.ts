import { afterEach, describe, expect, it, vi } from 'vitest';
import { fail, usageError } from '../../../src/cli/renderer/common.js';

// biome-ignore lint/suspicious/noControlCharactersInRegex: ANSI escapes are control chars
const stripAnsi = (s: string) => s.replace(/\[[0-9;]*m/g, '');

function capture() {
  const err = vi.spyOn(console, 'error').mockImplementation(() => {});
  const exit = vi.spyOn(process, 'exit').mockImplementation((() => {}) as never);
  return {
    text: () =>
      err.mock.calls
        .flat()
        .map((c) => stripAnsi(String(c)))
        .join('\n'),
    exit,
  };
}

afterEach(() => {
  vi.restoreAllMocks();
});

describe('fail', () => {
  it('prints the error message to stderr and exits 1', () => {
    const cap = capture();
    fail(new Error('connection refused'));
    expect(cap.text()).toContain('connection refused');
    expect(cap.exit).toHaveBeenCalledWith(1);
  });

  it('includes HTTP status and a --debug hint when the error carries one', () => {
    const cap = capture();
    fail(Object.assign(new Error('unprocessable entity'), { status: 422 }));
    const text = cap.text();
    expect(text).toContain('unprocessable entity');
    expect(text).toContain('HTTP 422');
    expect(text).toContain('--debug');
  });

  it('reads statusCode as an alternative status field', () => {
    const cap = capture();
    fail(Object.assign(new Error('nope'), { statusCode: 403 }));
    expect(cap.text()).toContain('HTTP 403');
  });

  it('stringifies non-Error values', () => {
    const cap = capture();
    fail('plain failure');
    expect(cap.text()).toContain('plain failure');
    expect(cap.exit).toHaveBeenCalledWith(1);
  });
});

describe('usageError', () => {
  it('prints the message and exits 2', () => {
    const cap = capture();
    usageError('--count must be a positive integer');
    expect(cap.text()).toContain('--count must be a positive integer');
    expect(cap.exit).toHaveBeenCalledWith(2);
  });
});
