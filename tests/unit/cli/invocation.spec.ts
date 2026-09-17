import { afterEach, describe, expect, it, vi } from 'vitest';
import { commandHint } from '../../../src/cli/invocation.js';
import { buildProgram } from '../../../src/cli/program.js';
import { noTenantSelectedMessage } from '../../../src/config/loader.js';

afterEach(() => vi.unstubAllEnvs());

describe('standalone and managed command names', () => {
  it('uses the standalone name and ignores arbitrary display contexts', () => {
    vi.stubEnv('AIRS_CLI_INVOKED_AS', 'untrusted --command');
    expect(buildProgram().helpInformation()).toContain('Usage: airs-cli');
    expect(noTenantSelectedMessage([])).toContain('airs-cli tenant create');
  });

  it('shows runnable managed usage, examples and tenant recovery', () => {
    vi.stubEnv('AIRS_CLI_INVOKED_AS', 'airs cli');
    const program = buildProgram();
    expect(program.helpInformation()).toContain('Usage: airs cli');
    const doctor = program.commands.find((command) => command.name() === 'doctor');
    const write = vi.spyOn(process.stdout, 'write').mockImplementation(() => true);
    doctor?.outputHelp();
    expect(write.mock.calls.map(([value]) => String(value)).join('')).toContain('airs cli doctor');
    write.mockRestore();
    expect(noTenantSelectedMessage([])).toContain('airs cli tenant create');
    expect(commandHint('Run airs-cli tenant switch work')).toBe('Run airs cli tenant switch work');
  });
});
