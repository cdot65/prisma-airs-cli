import { afterEach, describe, expect, it, vi } from 'vitest';
import { handleUnhandledRejection } from '../../../src/cli/process-guards.js';

describe('handleUnhandledRejection', () => {
  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('prints a friendly message to stderr and exits 1 for Error reasons', () => {
    const errSpy = vi.spyOn(console, 'error').mockImplementation(() => {});
    const exitSpy = vi.spyOn(process, 'exit').mockImplementation((() => {}) as never);

    handleUnhandledRejection(new Error('boom'));

    const output = errSpy.mock.calls.flat().join('\n');
    expect(output).toContain('boom');
    expect(output.toLowerCase()).toContain('unexpected');
    expect(exitSpy).toHaveBeenCalledWith(1);
  });

  it('stringifies non-Error reasons', () => {
    const errSpy = vi.spyOn(console, 'error').mockImplementation(() => {});
    vi.spyOn(process, 'exit').mockImplementation((() => {}) as never);

    handleUnhandledRejection('plain-string-reason');

    expect(errSpy.mock.calls.flat().join('\n')).toContain('plain-string-reason');
  });
});
