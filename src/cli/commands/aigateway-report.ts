import { lstat } from 'node:fs/promises';
import { resolve } from 'node:path';
import { AIGatewayClient } from '@cdot65/prisma-airs-sdk';
import type { Command } from 'commander';
import { z } from 'zod';
import { aiGatewayClientOptions } from '../../config/client-options.js';
import { loadConfig } from '../../config/loader.js';
import { collectGatewayEnvironmentReport } from '../../reports/aigateway.js';
import {
  renderEnvironmentReportHtml,
  renderEnvironmentReportMarkdown,
} from '../../reports/environment-render.js';
import { writeReportFile } from '../../reports/io.js';
import { examples } from '../examples.js';
import { ui, usageError } from '../renderer/index.js';

/** Register a read-only report with artifact formats independent of terminal output settings. */
export function registerAiGatewayReportCommand(aigateway: Command): void {
  const command = aigateway
    .command('report')
    .alias('dashboard')
    .description('Generate an AI Gateway environment report (read-only)')
    .requiredOption('--workspace <ref>', 'Workspace slug, UUID, or unique display name')
    .option('--days <n>', 'Rolling window in days (default: 1)', '1')
    .option('--start <iso>', 'Explicit ISO-8601 start; requires --end')
    .option('--end <iso>', 'Explicit ISO-8601 end; requires --start')
    .option('--output <format>', 'Deliverable format: html or markdown (default: html)')
    .option(
      '--output-file <path>',
      'Destination (default: new timestamped file in CWD); - for stdout; never overwrite',
    )
    .option('--title <text>', 'Report title', 'AI Gateway environment report')
    .option('--max-pages <n>', 'Page budget per source, 1–100 (50 transactions/page)', '40')
    .option('--strict', 'Exit 1 after writing if any source is incomplete')
    .addHelpText(
      'after',
      examples(
        'airs aigateway report --workspace ws-develo-71f8d8 --output-file ./airs-daily.html',
        'airs aigateway report --workspace ws-develo-71f8d8 --output markdown --output-file ./airs-daily.md',
        'airs aigateway report --workspace ws-develo-71f8d8',
        'airs aigateway report --workspace ws-develo-71f8d8 --strict --max-pages 20 --output-file - > daily.html',
      ),
    )
    .action(async (opts) => {
      const format = opts.output ?? command.parent?.parent?.opts().output ?? 'html';
      if (format !== 'html' && format !== 'markdown')
        usageError('Report output must be html or markdown');
      if (typeof opts.workspace !== 'string' || !opts.workspace.trim())
        usageError('--workspace cannot be empty');
      if (!/^[1-9]\d*$/.test(opts.days) || !Number.isSafeInteger(Number(opts.days)))
        usageError('--days must be a positive integer');
      if (Boolean(opts.start) !== Boolean(opts.end))
        usageError('--start and --end must be supplied together');
      if (opts.start && command.getOptionValueSource('days') !== 'default')
        usageError('Use --days or --start/--end, not both');
      const end = opts.end ? new Date(opts.end) : new Date();
      const start = opts.start
        ? new Date(opts.start)
        : new Date(end.getTime() - Number(opts.days) * 86400000);
      const iso = z.string().datetime({ offset: true });
      if (
        (opts.start && (!iso.safeParse(opts.start).success || !iso.safeParse(opts.end).success)) ||
        !Number.isFinite(start.getTime()) ||
        !Number.isFinite(end.getTime()) ||
        start >= end
      )
        usageError('Provide a valid increasing ISO-8601 window with timezone');
      const maxPages = Number(opts.maxPages);
      if (!/^[1-9]\d*$/.test(opts.maxPages) || !Number.isSafeInteger(maxPages) || maxPages > 100)
        usageError('--max-pages must be an integer from 1 to 100');
      if (typeof opts.title !== 'string' || !opts.title.trim() || opts.title.length > 240)
        usageError('--title must contain 1–240 characters');
      if (opts.outputFile !== undefined && !opts.outputFile.trim())
        usageError('--output-file cannot be empty');
      const defaultName = `airs-aigateway-report-${new Date().toISOString().replaceAll(':', '-')}-${randomUUID().slice(0, 8)}.${format === 'html' ? 'html' : 'md'}`;
      const outputPath =
        opts.outputFile === '-' ? undefined : resolve(opts.outputFile ?? defaultName);
      try {
        if (outputPath) {
          const existing = await lstat(outputPath).catch((error: NodeJS.ErrnoException) => {
            if (error.code !== 'ENOENT') throw error;
            return undefined;
          });
          if (existing) throw new Error('Output already exists');
        }
        const config = await loadConfig();
        const client = new AIGatewayClient({
          ...aiGatewayClientOptions(config),
          numRetries: 0,
        });
        ui.status('Collecting AI Gateway statistics and current configuration (read-only)...');
        const report = await collectGatewayEnvironmentReport(client, {
          title: opts.title,
          maxPages,
          workspace: opts.workspace,
          tsgId: config.mgmtTsgId ?? process.env.PANW_AI_GW_TSG_ID ?? '',
          start,
          end,
        });
        const rendered =
          format === 'html'
            ? renderEnvironmentReportHtml(report)
            : renderEnvironmentReportMarkdown(report);
        if (outputPath) {
          await writeReportFile(outputPath, rendered);
          ui.status(`Report → ${outputPath.replace(/[\p{Cc}\p{Cf}]/gu, '_')} (mode 0600)`);
        } else {
          await new Promise<void>((done, reject) =>
            process.stdout.write(rendered, (error) => (error ? reject(error) : done())),
          );
        }
        const incomplete = report.sources.filter((source) => source.status !== 'complete').length;
        if (incomplete)
          ui.status(`${incomplete} source(s) incomplete; review the evidence section.`);
        if (report.health === 'unknown' || (opts.strict && incomplete > 0)) process.exitCode = 1;
      } catch {
        // Never echo upstream/config errors or filesystem paths into a shareable stream.
        ui.error(
          'Report generation failed. Verify Management API configuration and a writable, new output path. Existing files are never overwritten.',
        );
        process.exitCode = 1;
      }
    });
}

import { randomUUID } from 'node:crypto';
