import { afterEach, describe, expect, it, vi } from 'vitest';
import { confirmOrAbort } from '../../../src/cli/confirm.js';

const ANSI = /\[[0-9;]*m/g;
const stripAnsi = (s: string) => s.replace(ANSI, '');

function mockExit() {
  return vi.spyOn(process, 'exit').mockImplementation(((code?: number) => {
    throw new Error(`process.exit(${code})`);
  }) as never);
}

afterEach(() => {
  vi.restoreAllMocks();
});

describe('confirmOrAbort', () => {
  it('proceeds immediately when force is true without prompting', async () => {
    const promptFn = vi.fn();
    await expect(
      confirmOrAbort('Delete thing?', true, { promptFn, isTTY: true }),
    ).resolves.toBeUndefined();
    expect(promptFn).not.toHaveBeenCalled();
  });

  it('exits 2 with a refusal message when not a TTY and not forced', async () => {
    const exit = mockExit();
    const errSpy = vi.spyOn(console, 'error').mockImplementation(() => {});
    const promptFn = vi.fn();
    await expect(
      confirmOrAbort('Delete thing?', false, {
        promptFn,
        isTTY: false,
        action: 'delete profile "demo"',
      }),
    ).rejects.toThrow('process.exit(2)');
    expect(exit).toHaveBeenCalledWith(2);
    expect(promptFn).not.toHaveBeenCalled();
    const text = errSpy.mock.calls
      .flat()
      .map((c) => stripAnsi(String(c)))
      .join('\n');
    expect(text).toContain('refusing to delete profile "demo" without --force');
    expect(text).toContain('non-interactive');
  });

  it('proceeds when TTY prompt is confirmed', async () => {
    const promptFn = vi.fn().mockResolvedValue(true);
    await expect(
      confirmOrAbort('Delete thing?', false, { promptFn, isTTY: true }),
    ).resolves.toBeUndefined();
    expect(promptFn).toHaveBeenCalledWith({ message: 'Delete thing?', default: false });
  });

  it('prints Aborted and exits 0 when TTY prompt is declined', async () => {
    const exit = mockExit();
    const logSpy = vi.spyOn(console, 'log').mockImplementation(() => {});
    const promptFn = vi.fn().mockResolvedValue(false);
    await expect(confirmOrAbort('Delete thing?', false, { promptFn, isTTY: true })).rejects.toThrow(
      'process.exit(0)',
    );
    expect(exit).toHaveBeenCalledWith(0);
    const text = logSpy.mock.calls
      .flat()
      .map((c) => stripAnsi(String(c)))
      .join('\n');
    expect(text).toContain('Aborted');
  });

  it('defaults the prompt answer to false (decline)', async () => {
    const promptFn = vi.fn().mockResolvedValue(true);
    await confirmOrAbort('Sure?', false, { promptFn, isTTY: true });
    expect(promptFn.mock.calls[0][0]).toMatchObject({ default: false });
  });
});
