import { randomUUID } from 'node:crypto';
import { lstat } from 'node:fs/promises';
import { resolve } from 'node:path';
import {
  AgentGuardClient,
  AgentGuardPageOptionsSchema,
  AgentGuardScanListOptionsSchema,
  AgentGuardStatsOptionsSchema,
} from '@cdot65/prisma-airs-sdk';
import type { Command } from 'commander';
import { dump } from 'js-yaml';
import { z } from 'zod';
import { agentGuardClientOptions } from '../../config/client-options.js';
import { loadConfig } from '../../config/loader.js';
import { collectAgentGuardReport } from '../../reports/agentguard.js';
import {
  renderEnvironmentReportHtml,
  renderEnvironmentReportMarkdown,
} from '../../reports/environment-render.js';
import { writeReportFile } from '../../reports/io.js';
import { formatOutput, resolveOutput, ui, usageError } from '../renderer/index.js';

const createClient = async () =>
  new AgentGuardClient({ ...agentGuardClientOptions(await loadConfig()), numRetries: 0 });
const integer = (value: string) => {
  if (!/^\d+$/.test(value) || !Number.isSafeInteger(Number(value)))
    usageError('Pagination requires a safe nonnegative integer');
  return Number(value);
};
const pageOptions = (cmd: Command) =>
  cmd
    .option('--limit <n>', 'Page size', integer, 10)
    .option('--offset <n>', 'Starting offset', integer, 0);
const readOutput = (cmd: Command) =>
  cmd.option('--output <format>', 'Output format: pretty, table, markdown, csv, json, yaml');
const query = <T>(schema: z.ZodType<T>, value: unknown): T => {
  const result = schema.safeParse(value);
  if (!result.success)
    usageError(
      'Invalid AgentGuard query. Check pagination, UUID and ISO timestamps; statistics support 30_DAYS only.',
    );
  return result.data;
};
function emit(value: Record<string, unknown>, format: string) {
  if (format === 'json') console.log(JSON.stringify(value, null, 2));
  else if (format === 'yaml') console.log(dump(value, { noRefs: true, lineWidth: -1 }).trimEnd());
  else {
    const records = Object.values(value).find(Array.isArray) as
      | Record<string, unknown>[]
      | undefined;
    if (records) {
      const keys = [...new Set(records.flatMap((r) => Object.keys(r)))]
        .filter((k) => !['description', 'remediation'].includes(k))
        .slice(0, 9);
      console.log(
        formatOutput(
          records,
          keys.map((key) => ({ key, label: key.replaceAll('_', ' ') })),
          format === 'pretty' ? 'table' : (format as 'table' | 'markdown' | 'csv'),
        ),
      );
      ui.status(`Returned ${records.length} records; see JSON/YAML for pagination metadata.`);
      return;
    }
    const rows = Object.entries(value).map(([key, v]) => ({
      key,
      value: typeof v === 'object' ? JSON.stringify(v) : String(v),
    }));
    console.log(
      formatOutput(
        rows,
        [
          { key: 'key', label: 'Field' },
          { key: 'value', label: 'Value' },
        ],
        format === 'pretty' ? 'table' : (format as 'table' | 'markdown' | 'csv'),
      ),
    );
  }
}
async function run(read: () => Promise<Record<string, unknown>>, cmd: Command) {
  try {
    const format = await resolveOutput(cmd, cmd.opts());
    emit(await read(), format);
  } catch (error) {
    const status = (error as { statusCode?: number })?.statusCode;
    ui.error(
      `AgentGuard request failed${typeof status === 'number' && Number.isInteger(status) && status >= 400 && status <= 599 ? ` (HTTP ${status})` : ''}. Check credentials, query options and service availability. No upstream body is included.`,
    );
    process.exitCode = 1;
  }
}

async function inventory<T extends { uuid: string }>(
  read: (skip: number, limit: number) => Promise<{ rows: T[]; total: number }>,
  options: { offset: number; limit: number; all?: boolean; max?: number },
  pageCount = false,
) {
  if (options.max !== undefined && (!Number.isSafeInteger(options.max) || options.max < 0))
    usageError('--max must be a safe nonnegative integer');
  const rows: T[] = [];
  const seen = new Set<string>();
  let total: number | undefined;
  const cap = options.all ? options.max || Number.MAX_SAFE_INTEGER : options.limit;
  for (let page = 0; page < 1000; page++) {
    const limit = Math.min(options.limit, cap - rows.length);
    const p = await read(options.offset + rows.length, limit);
    total ??= p.total;
    if ((!pageCount && p.total !== total) || p.rows.length > limit)
      throw new Error('Unstable pagination');
    for (const row of p.rows) {
      if (!row.uuid || seen.has(row.uuid)) throw new Error('Repeated identity');
      seen.add(row.uuid);
      rows.push(row);
    }
    const next = options.offset + rows.length;
    if (!pageCount && next > total && rows.length) throw new Error('Invalid total');
    const complete = pageCount ? p.rows.length < limit : next >= total;
    if (!options.all || complete || rows.length === cap)
      return {
        rows,
        pagination: {
          total_items: pageCount ? (complete && options.offset === 0 ? rows.length : null) : total,
          returned: rows.length,
          skip: options.offset,
          next_offset: complete ? null : next,
          truncated: !complete,
        },
      };
    if (!p.rows.length) throw new Error('Premature empty page');
  }
  throw new Error('Pagination safety limit reached');
}

/** Read-only agent/skill scanning telemetry. Report is the canonical deliverable command. */
export function registerAgentGuardCommand(program: Command): void {
  const group = program
    .command('agentguard')
    .description('AI Supply Chain agent and skill scan telemetry (experimental, OAuth)');
  const scans = group.command('scans').description('Inspect agent and skill scans (read-only)');
  const list = readOutput(
    pageOptions(
      scans.command('list').description('List one page of scans; sensitive metadata is excluded'),
    ),
  )
    .option('--start <iso>', 'Inclusive creation time start (ISO timestamp)')
    .option('--end <iso>', 'Creation time end (ISO timestamp)')
    .action(async (opts) => {
      const options = query(AgentGuardScanListOptionsSchema, {
        skip: opts.offset,
        limit: opts.limit,
        start_time: opts.start,
        end_time: opts.end,
        isBackgroundRefresh: false,
      });
      await run(async () => {
        const client = await createClient();
        const p = await inventory(async (skip, limit) => {
          const page = await client.listScans({ ...options, skip, limit });
          return { rows: page.scans, total: page.pagination.total_items };
        }, opts);
        return {
          pagination: p.pagination,
          scans: p.rows.map((r) => ({
            uuid: r.uuid,
            artifact_type: r.artifact_type,
            status: r.status,
            eval_outcome: r.eval_outcome,
            created_at: r.created_at,
            vulnerability_count: r.summary?.vulnerability_count ?? null,
            attack_chain_count: r.summary?.attack_chain_count ?? null,
            is_batch: r.is_batch,
            parent_uuid: r.parent_uuid,
          })),
        };
      }, list);
    });
  const vulnerabilities = readOutput(
    scans
      .command('vulnerabilities <scanUuid>')
      .description('List finding metadata; --include-content explicitly reveals sensitive details'),
  )
    .option('--include-content', 'Include source code, paths and finding text (sensitive)')
    .action(async (id: string, opts) => {
      query(z.string().uuid(), id);
      await run(async () => {
        const p = await (await createClient()).listScanVulnerabilities(id);
        return {
          pagination: p.pagination,
          vulnerabilities: p.vulnerabilities.map((r) =>
            opts.includeContent
              ? r
              : {
                  uuid: r.uuid,
                  scan_uuid: r.scan_uuid,
                  vulnerability_type: r.vulnerability_type,
                  attack_chain_count: r.attack_chain_count,
                },
          ),
          content_included: !!opts.includeContent,
        };
      }, vulnerabilities);
    });
  const stats = readOutput(
    group.command('stats').description('Read server rolling 30-day skill statistics'),
  )
    .option('--time-period <period>', 'Verified period: 30_DAYS', '30_DAYS')
    .action(async (opts) => {
      const options = query(AgentGuardStatsOptionsSchema, { time_period: opts.timePeriod });
      await run(async () => (await createClient()).getScanStats(options), stats);
    });
  const rulesGroup = group
    .command('rules')
    .description('Rule catalog (not effective tenant policy)');
  const rules = readOutput(
    pageOptions(rulesGroup.command('list').description('List rule catalog defaults')),
  ).action(async (opts) => {
    const options = query(AgentGuardPageOptionsSchema, { skip: opts.offset, limit: opts.limit });
    await run(async () => {
      const client = await createClient();
      const p = await inventory(
        async (skip, limit) => {
          const page = await client.listRules({ ...options, skip, limit });
          return { rows: page.rules, total: page.pagination.total_items };
        },
        opts,
        true,
      );
      return { pagination: p.pagination, rules: p.rows };
    }, rules);
  });

  group
    .command('report')
    .description('Generate an aggregate-only 30-day HTML/Markdown report')
    .option('--output <format>', 'Deliverable: html (default) or markdown')
    .option('--output-file <path>', 'New file in CWD by default; - for stdout; never overwrite')
    .option('--max-pages <n>', 'Page budget per inventory (1–100, 10 rows/page)', integer, 40)
    .option('--title <text>', 'Report title', 'AgentGuard environment report')
    .option('--strict', 'Exit 1 after writing if any source is incomplete')
    .action(async (opts) => {
      const format = opts.output ?? program.opts().output ?? 'html';
      if (!['html', 'markdown'].includes(format))
        usageError('Report output must be html or markdown');
      if (opts.maxPages < 1 || opts.maxPages > 100)
        usageError('--max-pages must be between 1 and 100');
      if (!opts.title.trim() || opts.title.length > 240)
        usageError('--title must contain 1–240 characters');
      if (opts.outputFile !== undefined && !opts.outputFile.trim())
        usageError('--output-file cannot be empty');
      const path =
        opts.outputFile === '-'
          ? undefined
          : resolve(
              opts.outputFile ??
                `airs-agentguard-report-${new Date().toISOString().replaceAll(':', '-')}-${randomUUID().slice(0, 8)}.${format === 'html' ? 'html' : 'md'}`,
            );
      try {
        if (path) {
          const exists = await lstat(path).catch((e: NodeJS.ErrnoException) => {
            if (e.code !== 'ENOENT') throw e;
            return undefined;
          });
          if (exists) throw new Error();
        }
        const data = await collectAgentGuardReport(await createClient(), {
          title: opts.title,
          maxPages: opts.maxPages,
        });
        const output =
          format === 'html'
            ? renderEnvironmentReportHtml(data)
            : renderEnvironmentReportMarkdown(data);
        if (path) {
          await writeReportFile(path, output);
          ui.status(`Report → ${path.replace(/[\p{Cc}\p{Cf}]/gu, '_')} (mode 0600)`);
        } else
          await new Promise<void>((done, reject) =>
            process.stdout.write(output, (error) => (error ? reject(error) : done())),
          );
        const incomplete = data.sources.filter((s) => s.status !== 'complete').length;
        if (incomplete)
          ui.status(`${incomplete} source(s) incomplete; review the evidence section.`);
        if (data.health === 'unknown' || (opts.strict && incomplete)) process.exitCode = 1;
      } catch {
        ui.error(
          'AgentGuard report failed. Check credentials and a writable, new output path. Existing files are never overwritten.',
        );
        process.exitCode = 1;
      }
    });
}
