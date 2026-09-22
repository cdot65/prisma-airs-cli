import { Command } from 'commander';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { register } from '../../../src/cli/commands/dlp/profiles.js';

afterEach(() => vi.restoreAllMocks());

describe('DLP profile deletion safety', () => {
  it('exits 2 without network traffic and does not promise a working soft-delete', async () => {
    const command = new Command('dlp');
    register(command);
    const network = vi.spyOn(globalThis, 'fetch').mockRejectedValue(new Error('Unexpected HTTP'));
    const output = vi.spyOn(console, 'error').mockImplementation(() => {});
    const exit = vi.spyOn(process, 'exit').mockImplementation(() => {
      throw new Error('exit');
    });
    await expect(
      command.parseAsync(['profiles', 'delete', '123'], { from: 'user' }),
    ).rejects.toThrow('exit');
    expect(exit).toHaveBeenCalledWith(2);
    expect(network).not.toHaveBeenCalled();
    expect(output.mock.calls.flat().join(' ')).toContain('not live-verified');
    expect(output.mock.calls.flat().join(' ')).toContain('No API request was sent');
    expect(output.mock.calls.flat().join(' ')).not.toContain('--set profile_status');
  });
});
