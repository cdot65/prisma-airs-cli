import { mkdtemp, readFile, rm, stat, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { Command } from 'commander';
import { load } from 'js-yaml';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { TEST_CREDENTIALS, useTestTenant } from '../../helpers/tenant.js';

const factory = vi.hoisted(() => ({ getOrCreateManagementClient: vi.fn() }));
vi.mock('../../../src/airs/management.js', () => factory);

import { registerRuntimeDashboardCommands } from '../../../src/cli/commands/runtime-dashboard.js';

describe('runtime dashboard/session commands', () => {
  let directory: string;
  let tenant: Awaited<ReturnType<typeof useTestTenant>>;
  const names = [
    'applicationsOverview',
    'application',
    'applicationViolationBreakdown',
    'topApplicationsViolations',
    'applicationsViolationsTrend',
    'appsList',
    'sessionsChart',
    'sessionsOverview',
    'session',
    'sessionTransaction',
    'scanContent',
  ];
  let methods: Record<string, ReturnType<typeof vi.fn>>;
  let output: ReturnType<typeof vi.spyOn>;
  const entry = {
    application_id: 'app',
    application_name: 'A & B',
    session_id: 'pan_session',
    violation_status: 'passed',
  };
  const page = (items = [entry], total = items.length, skip = 0) => ({
    items,
    pagination: { total_items: total, skip, limit: 25 },
  });
  const beforeAction = vi.fn();
  const run = (args: string[], format = 'json') => {
    const program = new Command();
    program.hook('preAction', beforeAction);
    registerRuntimeDashboardCommands(program.command('runtime'));
    return program.parseAsync(['node', 'airs', 'runtime', ...args, '--output', format]);
  };
  beforeEach(async () => {
    vi.clearAllMocks();
    process.exitCode = undefined;
    directory = await mkdtemp(join(tmpdir(), 'airs-dashboard-command-'));
    tenant = await useTestTenant({ defaultOutput: 'json' });
    methods = Object.fromEntries(
      names.map((name) => [name, vi.fn().mockResolvedValue({ result: name })]),
    );
    methods.sessionsOverview.mockResolvedValue(page());
    factory.getOrCreateManagementClient.mockReturnValue({ dashboard: methods });
    output = vi.spyOn(console, 'log').mockImplementation(() => {});
    vi.spyOn(console, 'error').mockImplementation(() => {});
  });
  afterEach(async () => {
    process.exitCode = undefined;
    vi.unstubAllEnvs();
    vi.restoreAllMocks();
    await rm(directory, { recursive: true, force: true });
  });

  it.each([
    [
      'applications',
      'applicationsOverview',
      { timeInterval: 1, timeUnit: 'day', limit: 25, offset: 0 },
    ],
    ['top-applications', 'topApplicationsViolations', { timeInterval: 1, timeUnit: 'day' }],
    ['violations-trend', 'applicationsViolationsTrend', { timeInterval: 1, timeUnit: 'day' }],
    ['apps-list', 'appsList', { timeInterval: 30, timeUnit: 'days' }],
  ])('wires dashboard %s with exact defaults', async (name, method, query) => {
    await run(['dashboard', String(name)]);
    expect(methods[String(method)]).toHaveBeenCalledWith(query);
    expect(process.exitCode).toBeUndefined();
  });
  it.each([
    ['application', 'application'],
    ['application-violations', 'applicationViolationBreakdown'],
  ])('maps %s identity and detail window', async (name, method) => {
    await run(['dashboard', name, '--app-id', 'app', '--app-name', 'A & B', '--interval', '7']);
    expect(methods[method]).toHaveBeenCalledWith({
      appId: 'app',
      appName: 'A & B',
      timeInterval: 7,
      timeUnit: 'days',
    });
  });
  it('maps chart and paginated session list, outputting a bare array', async () => {
    await run(['sessions', 'chart']);
    expect(methods.sessionsChart).toHaveBeenCalledWith({ timeInterval: 1, timeUnit: 'day' });
    await run(['sessions', 'list', '--offset', '2']);
    expect(methods.sessionsOverview).toHaveBeenCalledWith({
      timeInterval: 1,
      timeUnit: 'day',
      limit: 25,
      offset: 2,
    });
    expect(JSON.parse(String(output.mock.calls.at(-1)?.[0]))).toEqual([entry]);
  });
  const identityArgs = ['--session-id', 'pan_session', '--app-id', 'app', '--app-name', 'A & B'];
  it.each([
    'json',
    'yaml',
  ])('rejects week before configuration/authentication (%s)', async (format) => {
    // Invalid config would fail if option validation did not happen first.
    await writeFile(
      tenant.configPath,
      JSON.stringify({ ...TEST_CREDENTIALS, scanConcurrency: 'invalid' }),
    );
    const exit = vi.spyOn(process, 'exit').mockImplementation(() => {
      throw new Error('exit');
    });
    await expect(
      run(['sessions', 'list', '--interval', '1', '--unit', 'week'], format),
    ).rejects.toThrow('exit');
    expect(exit).toHaveBeenCalledWith(2);
    expect(beforeAction).not.toHaveBeenCalled();
    expect(factory.getOrCreateManagementClient).not.toHaveBeenCalled();
    expect(methods.sessionsOverview).not.toHaveBeenCalled();
    expect(output).not.toHaveBeenCalled();
    const stderr = JSON.stringify(vi.mocked(console.error).mock.calls);
    expect(stderr).toContain('Supported units: hour, hours, day, days');
    expect(stderr).toContain('--interval 7 --unit days');
    expect(stderr).not.toContain('Dashboard operation failed');
  });
  it.each([
    'weeks',
    'month',
    'months',
    'year',
    'years',
    'fortnight',
    'DAY',
    ' day ',
  ])('rejects unsupported unit %s before authentication', async (unit) => {
    const exit = vi.spyOn(process, 'exit').mockImplementation(() => {
      throw new Error('exit');
    });
    await expect(run(['sessions', 'list', '--unit', unit])).rejects.toThrow('exit');
    expect(exit).toHaveBeenCalledWith(2);
    expect(factory.getOrCreateManagementClient).not.toHaveBeenCalled();
    expect(output).not.toHaveBeenCalled();
  });
  it.each([
    ['sessions', 'chart'],
    ['sessions', 'get', ...identityArgs],
    ['sessions', 'transaction', ...identityArgs, '--scan-id', 'scan', '--scan-sub-req-id', '0'],
    ['dashboard', 'top-applications'],
    ['dashboard', 'violations-trend'],
    ['dashboard', 'apps-list'],
  ])('rejects unsupported units consistently for %j', async (...args) => {
    const exit = vi.spyOn(process, 'exit').mockImplementation(() => {
      throw new Error('exit');
    });
    await expect(run([...args, '--unit', 'week'])).rejects.toThrow('exit');
    expect(exit).toHaveBeenCalledWith(2);
    expect(factory.getOrCreateManagementClient).not.toHaveBeenCalled();
    expect(JSON.stringify(vi.mocked(console.error).mock.calls)).toContain(
      '--interval 7 --unit days',
    );
  });
  it.each([
    ['1', 'hour'],
    ['24', 'hours'],
    ['1', 'day'],
    ['7', 'days'],
  ])('preserves the supported %s %s query and YAML output', async (interval, unit) => {
    await run(['sessions', 'list', '--interval', interval, '--unit', unit], 'yaml');
    expect(methods.sessionsOverview).toHaveBeenCalledWith({
      timeInterval: Number(interval),
      timeUnit: unit,
      limit: 25,
      offset: 0,
    });
    expect(load(String(output.mock.calls[0][0]))).toEqual([entry]);
    expect(process.exitCode).toBeUndefined();
  });
  it('advertises the supported units in session help', () => {
    const runtime = new Command('runtime');
    registerRuntimeDashboardCommands(runtime);
    const list = runtime.commands
      .find((command) => command.name() === 'sessions')
      ?.commands.find((command) => command.name() === 'list');
    expect(list?.helpInformation()).toContain('Time unit: hour, hours, day, days');
  });
  it('maps detail and transaction, retaining sub-request zero and default 30-day window', async () => {
    await run(['sessions', 'get', ...identityArgs]);
    expect(methods.session).toHaveBeenCalledWith({
      sessionId: 'pan_session',
      appId: 'app',
      appName: 'A & B',
      timeInterval: 30,
      timeUnit: 'days',
      limit: 25,
      offset: 0,
    });
    await run([
      'sessions',
      'transaction',
      ...identityArgs,
      '--scan-id',
      'scan',
      '--scan-sub-req-id',
      '0',
    ]);
    expect(methods.sessionTransaction.mock.calls[0][0].scanSubReqId).toBe(0);
    expect(methods.scanContent).not.toHaveBeenCalled();
  });
  it('walks all pages and preserves same-name identities', async () => {
    methods.sessionsOverview
      .mockResolvedValueOnce(page([entry], 2))
      .mockResolvedValueOnce(page([{ ...entry, application_id: 'other' }], 2, 1));
    await run(['sessions', 'list', '--all']);
    expect(methods.sessionsOverview.mock.calls[1][0].offset).toBe(1);
    expect(JSON.parse(String(output.mock.calls[0][0]))).toHaveLength(2);
    expect(process.exitCode).toBeUndefined();
  });
  it.each([
    page([entry], 3, 1),
    page([entry], 2, 1),
    page([], 2, 1),
    page([entry], 0, 1),
  ])('does not emit a successful inventory for unstable pages', async (later) => {
    methods.sessionsOverview.mockResolvedValueOnce(page([entry], 2)).mockResolvedValueOnce(later);
    await run(['sessions', 'list', '--all']);
    expect(process.exitCode).toBe(1);
    expect(output).not.toHaveBeenCalled();
  });
  it('marks capped lists partial and returns only the cap', async () => {
    methods.sessionsOverview.mockResolvedValue(
      page([entry, { ...entry, session_id: 'second' }], 3),
    );
    await run(['sessions', 'list', '--all', '--max', '1']);
    expect(process.exitCode).toBe(1);
    expect(JSON.parse(String(output.mock.calls[0][0]))).toHaveLength(1);
  });
  it.each([
    ['sessions', 'list', '--limit', '0'],
    ['sessions', 'list', '--offset', '-1'],
    ['sessions', 'list', '--max', 'NaN'],
    ['sessions', 'chart', '--interval', '1oops'],
    ['dashboard', 'application', '--app-id', 'app', '--app-name', 'name', '--interval', '1'],
    ['sessions', 'scan-content', '--scan-id', 'scan', '--scan-sub-req-id', '0'],
    [
      'sessions',
      'scan-content',
      '--scan-id',
      'scan',
      '--scan-sub-req-id',
      '0',
      '--show-content',
      '--output-file',
      'file',
    ],
  ])('validates before authentication: %j', async (...args) => {
    const exit = vi.spyOn(process, 'exit').mockImplementation(() => {
      throw new Error('exit');
    });
    await expect(run(args)).rejects.toThrow('exit');
    expect(exit).toHaveBeenCalledWith(2);
    expect(factory.getOrCreateManagementClient).not.toHaveBeenCalled();
  });
  it('protects content exports and refuses to clobber files before authentication', async () => {
    methods.scanContent.mockResolvedValue({ scan_contents: { response: 'sensitive fixture' } });
    const path = join(directory, 'scan.json');
    const args = [
      'sessions',
      'scan-content',
      '--scan-id',
      'scan',
      '--scan-sub-req-id',
      '0',
      '--output-file',
      path,
    ];
    await run(args);
    expect(output).not.toHaveBeenCalled();
    expect((await stat(path)).mode & 0o777).toBe(0o600);
    expect(JSON.parse(await readFile(path, 'utf8')).scan_contents.response).toBe(
      'sensitive fixture',
    );
    factory.getOrCreateManagementClient.mockClear();
    await run(args);
    expect(process.exitCode).toBe(1);
    expect(factory.getOrCreateManagementClient).not.toHaveBeenCalled();
  });
  it('requires explicit display and sanitizes upstream failures', async () => {
    await run([
      'sessions',
      'scan-content',
      '--scan-id',
      'scan',
      '--scan-sub-req-id',
      '0',
      '--show-content',
    ]);
    expect(methods.scanContent).toHaveBeenCalledWith({ scanId: 'scan', scanSubReqId: 0 });
    methods.sessionsChart.mockRejectedValue(
      Object.assign(new Error('SECRET BODY'), { statusCode: 403 }),
    );
    await run(['sessions', 'chart']);
    expect(process.exitCode).toBe(1);
    expect(JSON.stringify(vi.mocked(console.error).mock.calls)).not.toContain('SECRET BODY');
  });
  it('uses dashboard-only override without changing the configured management host', async () => {
    await writeFile(
      tenant.configPath,
      JSON.stringify({
        ...TEST_CREDENTIALS,
        mgmtEndpoint: 'https://management.example.test',
        mgmtDashboardEndpoint: 'https://dashboard.example.test/aisec',
      }),
    );
    await run(['sessions', 'chart']);
    expect(factory.getOrCreateManagementClient.mock.calls[0][0]).toMatchObject({
      apiEndpoint: 'https://management.example.test',
      dashboardEndpoint: 'https://dashboard.example.test/aisec',
      numRetries: 0,
    });
  });
});
