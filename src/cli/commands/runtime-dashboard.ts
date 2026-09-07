import { lstat } from 'node:fs/promises';
import { resolve } from 'node:path';
import type { DashboardClient } from '@cdot65/prisma-airs-sdk';
import type { Command } from 'commander';
import { dump } from 'js-yaml';
import { getOrCreateManagementClient } from '../../airs/management.js';
import { managementClientOptions } from '../../config/client-options.js';
import { loadConfig } from '../../config/loader.js';
import { writeReportFile } from '../../reports/io.js';
import { CliUsageError, resolveOutput, ui, usageError } from '../renderer/index.js';

type Options = Record<string, string | boolean | undefined>;

const TIME_UNITS = ['hour', 'hours', 'day', 'days'] as const;

function integer(value: unknown, name: string, minimum = 0): number {
  const n = Number(value);
  if (!/^\d+$/.test(String(value)) || !Number.isSafeInteger(n) || n < minimum)
    throw new CliUsageError(`${name} must be a safe integer >= ${minimum}`);
  return n;
}

function text(value: unknown, name: string): string {
  if (typeof value !== 'string' || !value.trim()) throw new CliUsageError(`${name} is required`);
  return value;
}

function window(opts: Options) {
  return {
    timeInterval: integer(opts.interval, '--interval', 1),
    timeUnit: text(opts.unit, '--unit'),
  };
}

function identity(opts: Options) {
  return {
    sessionId: text(opts.sessionId, '--session-id'),
    appId: text(opts.appId, '--app-id'),
    appName: text(opts.appName, '--app-name'),
    ...window(opts),
  };
}

function scan(opts: Options) {
  return {
    scanId: text(opts.scanId, '--scan-id'),
    scanSubReqId: integer(opts.scanSubReqId, '--scan-sub-req-id'),
  };
}

function timeOptions(
  command: Command,
  interval = '1',
  unit = 'day',
  allowedUnits: readonly string[] = TIME_UNITS,
): Command {
  return command.option('--interval <n>', 'Look-back interval', interval).option(
    '--unit <unit>',
    `Time unit: ${allowedUnits.join(', ')}`,
    (value: string) => {
      // Parse before root preAction hooks can load config or create a debug file.
      if (!allowedUnits.includes(value))
        usageError(
          `Unsupported --unit. Supported units: ${allowedUnits.join(', ')}. For one week, use --interval 7 --unit days.`,
        );
      return value;
    },
    unit,
  );
}

function pageOptions(command: Command): Command {
  return command
    .option('--limit <n>', 'Items per page', '25')
    .option('--offset <n>', 'Item offset', '0');
}

function sessionOptions(command: Command): Command {
  return timeOptions(
    command
      .requiredOption('--session-id <id>', 'Session identity')
      .requiredOption('--app-id <id>', 'Application identity')
      .requiredOption('--app-name <name>', 'Exact dashboard application name'),
    '30',
    'days',
  );
}

function scanOptions(command: Command): Command {
  return command
    .requiredOption('--scan-id <id>', 'Scan identity')
    .requiredOption('--scan-sub-req-id <n>', 'Sub-request index, including zero');
}

/** Read-only SCM views. Output is JSON/YAML or escaped JSON for human inspection. */
export function registerRuntimeDashboardCommands(runtime: Command): void {
  const dashboard = runtime
    .command('dashboard')
    .description('SCM application summaries and daily telemetry (OAuth)');
  const sessions = runtime
    .command('sessions')
    .description('Verified SCM session inventory and drill-down (OAuth)');
  function register(
    command: Command,
    prepare: (opts: Options) => (client: DashboardClient) => Promise<unknown>,
  ) {
    command
      .option('--output <format>', 'Output format: pretty, json, yaml')
      .action(async (opts: Options) => {
        try {
          // Validate arguments before creating a client or obtaining OAuth credentials.
          const read = prepare(opts);
          const format = await resolveOutput(command, opts as { output?: string }, {
            allowed: ['pretty', 'json', 'yaml'],
          });
          if (command.name() === 'scan-content') {
            if (Boolean(opts.showContent) === Boolean(opts.outputFile))
              throw new CliUsageError(
                'Scan content requires exactly one of --show-content or --output-file <new-path>',
              );
            if (opts.outputFile) {
              const path = resolve(text(opts.outputFile, '--output-file'));
              const existing = await lstat(path).catch((error: NodeJS.ErrnoException) => {
                if (error.code !== 'ENOENT') throw error;
                return undefined;
              });
              if (existing) throw new Error('Destination exists');
            }
          }
          const config = await loadConfig();
          const client = getOrCreateManagementClient({
            ...managementClientOptions(config),
            numRetries: 0,
          });
          const result = await read(client.dashboard);
          const output =
            format === 'yaml'
              ? dump(result, { noRefs: true, lineWidth: -1 }).trimEnd()
              : JSON.stringify(result, null, 2);
          if (command.name() === 'scan-content' && opts.outputFile) {
            await writeReportFile(resolve(String(opts.outputFile)), `${output}\n`);
            ui.status('Scan content exported to the requested private file (0600).');
          } else console.log(output);
        } catch (error) {
          if (error instanceof CliUsageError) usageError(error.message);
          const status = (error as { statusCode?: number } | null)?.statusCode;
          ui.error(
            `Dashboard operation failed${typeof status === 'number' && status >= 400 && status <= 599 ? ` (HTTP ${status})` : ''}. Check credentials, query options, and any output destination. No existing file is overwritten.`,
          );
          process.exitCode = 1;
        }
      });
  }
  register(
    pageOptions(
      timeOptions(
        dashboard.command('applications').description('One page of application activity'),
        '1',
        'day',
        ['day', 'days', 'hour'],
      ),
    ),
    (opts) => {
      const interval = integer(opts.interval, '--interval', 1);
      const unit = text(opts.unit, '--unit');
      if (![1, 7, 30, 60].includes(interval) || !['day', 'days', 'hour'].includes(unit))
        throw new CliUsageError(
          'Application overview supports interval 1/7/30/60 and unit day/days/hour',
        );
      const query = {
        timeInterval: interval as 1 | 7 | 30 | 60,
        timeUnit: unit as 'day' | 'days' | 'hour',
        limit: integer(opts.limit, '--limit', 1),
        offset: integer(opts.offset, '--offset'),
      };
      return (client) => client.applicationsOverview(query);
    },
  );
  for (const [name, method] of [
    ['application', 'application'],
    ['application-violations', 'applicationViolationBreakdown'],
  ] as const) {
    register(
      timeOptions(
        dashboard
          .command(name)
          .requiredOption('--app-id <id>', 'Application identity')
          .requiredOption('--app-name <name>', 'Exact dashboard application name'),
        '30',
        'days',
        ['days'],
      ),
      (opts) => {
        const interval = integer(opts.interval, '--interval', 1);
        if (![7, 30, 60].includes(interval) || opts.unit !== 'days')
          throw new CliUsageError('Application detail requires 7, 30, or 60 days');
        const query = {
          appId: text(opts.appId, '--app-id'),
          appName: text(opts.appName, '--app-name'),
          timeInterval: interval as 7 | 30 | 60,
          timeUnit: 'days' as const,
        };
        return (client) => client[method](query);
      },
    );
  }
  for (const [name, method, days] of [
    ['top-applications', 'topApplicationsViolations', false],
    ['violations-trend', 'applicationsViolationsTrend', false],
    ['apps-list', 'appsList', true],
  ] as const) {
    register(
      timeOptions(dashboard.command(name), days ? '30' : '1', days ? 'days' : 'day'),
      (opts) => {
        const query = window(opts);
        return (client) => client[method](query);
      },
    );
  }
  register(timeOptions(sessions.command('chart')), (opts) => {
    const query = window(opts);
    return (client) => client.sessionsChart(query);
  });
  register(
    pageOptions(
      timeOptions(
        sessions
          .command('list')
          .description('Session entries; --all walks pages with completeness checks'),
      ),
    )
      .option('--all', 'Walk all pages (up to 100 pages)')
      .option('--max <n>', 'Item cap with --all; 0 removes item cap', '10000'),
    (opts) => {
      const query = {
        ...window(opts),
        limit: integer(opts.limit, '--limit', 1),
        offset: integer(opts.offset, '--offset'),
      };
      const maximum = integer(opts.max, '--max');
      return async (client) => {
        const items: Awaited<ReturnType<DashboardClient['sessionsOverview']>>['items'] = [];
        const seen = new Set<string>();
        let expectedTotal: number | undefined;
        let offset = query.offset;
        for (let pageIndex = 0; pageIndex < 100; pageIndex++) {
          const page = await client.sessionsOverview({ ...query, offset });
          if (!opts.all) {
            ui.status(
              `Returned ${page.items.length} of ${page.pagination.total_items ?? 'unknown'} sessions; this is one page.`,
            );
            return page.items;
          }
          const total = page.pagination.total_items;
          if (
            !Number.isSafeInteger(total) ||
            (total as number) < 0 ||
            page.pagination.skip !== offset ||
            page.pagination.limit !== query.limit ||
            page.items.length > query.limit ||
            (expectedTotal !== undefined && total !== expectedTotal)
          )
            throw new Error('Unstable pagination');
          expectedTotal = total;
          for (const item of page.items) {
            const key = JSON.stringify([
              item.application_id,
              item.application_name,
              item.session_id,
            ]);
            if (seen.has(key)) throw new Error('Overlapping session pages');
            seen.add(key);
            items.push(item);
          }
          offset += page.items.length;
          if (offset > (total as number) || (page.items.length === 0 && offset < (total as number)))
            throw new Error('Incomplete inventory');
          if (maximum > 0 && items.length > maximum) {
            ui.status('Session item cap reached; output is partial.');
            process.exitCode = 1;
            return items.slice(0, maximum);
          }
          if (offset === total) {
            ui.status(
              `Retrieved ${items.length} sessions from offset ${query.offset}; pagination complete.`,
            );
            return items;
          }
          if (maximum > 0 && items.length === maximum) {
            ui.status('Session item cap reached; output is partial.');
            process.exitCode = 1;
            return items;
          }
        }
        ui.status('Session page cap reached; output is partial.');
        process.exitCode = 1;
        return items;
      };
    },
  );
  register(pageOptions(sessionOptions(sessions.command('get'))), (opts) => {
    const query = {
      ...identity(opts),
      limit: integer(opts.limit, '--limit', 1),
      offset: integer(opts.offset, '--offset'),
    };
    return (client) => client.session(query);
  });
  register(scanOptions(sessionOptions(sessions.command('transaction'))), (opts) => {
    const query = { ...identity(opts), ...scan(opts) };
    return (client) => client.sessionTransaction(query);
  });
  register(
    scanOptions(
      sessions
        .command('scan-content')
        .description('Explicit sensitive scan-content read; never automatically fetched'),
    )
      .option('--show-content', 'Explicitly display stored content on stdout')
      .option('--output-file <path>', 'Private new file for stored content (no overwrite)'),
    (opts) => {
      const query = scan(opts);
      return (client) => client.scanContent(query);
    },
  );
}
