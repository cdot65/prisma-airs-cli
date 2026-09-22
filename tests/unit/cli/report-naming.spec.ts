import { Command } from 'commander';
import { describe, expect, it, vi } from 'vitest';
import { registerRedTeamReportCommand } from '../../../src/cli/commands/redteam-report.js';
import { buildProgram } from '../../../src/cli/program.js';

describe('canonical report naming', () => {
  it.each(['runtime', 'redteam', 'aigateway'])('exposes %s report', (product) => {
    const group = buildProgram().commands.find((command) => command.name() === product);
    if (!group) throw new Error(`Missing ${product} command`);
    const reports = group.commands.filter((command) => command.name() === 'report');
    expect(reports).toHaveLength(1);
    expect(reports[0].description()).not.toContain('dashboard');
    if (product !== 'runtime') expect(reports[0].aliases()).toContain('dashboard');
  });

  it('leaves runtime dashboard as the separate API-query group', () => {
    const runtime = buildProgram().commands.find((command) => command.name() === 'runtime');
    if (!runtime) throw new Error('Missing runtime command');
    expect(
      runtime.commands.find((command) => command.name() === 'dashboard')?.commands.length,
    ).toBeGreaterThan(0);
  });

  it('preserves legacy job-ID dispatch and attack options', async () => {
    const redteam = new Command('redteam');
    const scanReport = vi.fn().mockResolvedValue(undefined);
    registerRedTeamReportCommand(redteam, scanReport);
    await redteam.parseAsync([
      'node',
      'redteam',
      'report',
      'job-123',
      '--attacks',
      '--severity',
      'HIGH',
      '--limit',
      '7',
    ]);
    expect(scanReport).toHaveBeenCalledExactlyOnceWith(
      'job-123',
      expect.objectContaining({
        attacks: true,
        severity: 'HIGH',
        limit: '7',
      }),
    );
  });
});
