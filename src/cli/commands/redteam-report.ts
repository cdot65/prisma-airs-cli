import { lstat } from 'node:fs/promises';
import { resolve } from 'node:path';
import { RedTeamClient } from '@cdot65/prisma-airs-sdk';
import type { Command, OptionValues } from 'commander';
import { redTeamClientOptions } from '../../config/client-options.js';
import { loadConfig } from '../../config/loader.js';
import { writeReportFile } from '../../reports/io.js';
import { collectRedTeamEnvironmentReport } from '../../reports/redteam.js';
import {
  renderRedTeamReportHtml,
  renderRedTeamReportMarkdown,
} from '../../reports/redteam-render.js';
import { examples } from '../examples.js';
import { ui, usageError } from '../renderer/index.js';

/** Register a read-only report with artifact formats independent of terminal output settings. */
export function registerRedTeamReportCommand(
  redteam: Command,
  scanReport: (jobId: string, opts: OptionValues) => Promise<void>,
): void {
  const command = redteam
    .command('report [jobId]')
    .alias('dashboard')
    .description('Generate an environment report, or view an individual scan report by job ID')
    .option('--attacks', 'Individual scan only: include attack list', false)
    .option('--severity <level>', 'Individual scan only: filter attacks by severity')
    .option('--limit <n>', 'Individual scan only: max attacks to show', '20')
    .option('--output <format>', 'Deliverable format: html or markdown (default: html)')
    .option(
      '--output-file <path>',
      'Destination (default: new timestamped file in CWD); - for stdout; never overwrite',
    )
    .option('--title <text>', 'Report title', 'Red Team environment report')
    .option('--max-pages <n>', 'Page budget per source, 1–100 (15 records/page)', '40')
    .option('--strict', 'Exit 1 after writing if any source is incomplete')
    .addHelpText(
      'after',
      examples(
        'airs redteam report --output-file ./airs-daily.html',
        'airs redteam report --output markdown --output-file ./airs-daily.md',
        'airs redteam report',
        'airs redteam report --strict --max-pages 20 --output-file - > daily.html',
      ),
    )
    .action(async (jobId: string | undefined, opts) => {
      const explicit = (name: string) => command.getOptionValueSource(name) === 'cli';
      if (jobId !== undefined) {
        if (
          ['output', 'outputFile', 'title', 'maxPages', 'strict'].some(explicit) ||
          ['html', 'markdown'].includes(command.parent?.parent?.opts().output)
        )
          usageError('Environment report options cannot be combined with a scan job ID');
        await scanReport(jobId, opts);
        return;
      }
      if (['attacks', 'severity', 'limit'].some(explicit))
        usageError('--attacks, --severity and --limit require a scan job ID');
      const format = opts.output ?? command.parent?.parent?.opts().output ?? 'html';
      if (format !== 'html' && format !== 'markdown')
        usageError('Report output must be html or markdown');
      const maxPages = Number(opts.maxPages);
      if (!/^[1-9]\d*$/.test(opts.maxPages) || !Number.isSafeInteger(maxPages) || maxPages > 100)
        usageError('--max-pages must be an integer from 1 to 100');
      if (typeof opts.title !== 'string' || !opts.title.trim() || opts.title.length > 240)
        usageError('--title must contain 1–240 characters');
      if (opts.outputFile !== undefined && !opts.outputFile.trim())
        usageError('--output-file cannot be empty');
      const defaultName = `airs-redteam-report-${new Date().toISOString().replaceAll(':', '-')}-${randomUUID().slice(0, 8)}.${format === 'html' ? 'html' : 'md'}`;
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
        const client = new RedTeamClient({
          ...redTeamClientOptions(config),
          numRetries: 0,
        });
        ui.status('Collecting Red Team statistics and current configuration (read-only)...');
        const report = await collectRedTeamEnvironmentReport(client, {
          title: opts.title,
          maxPages,
        });
        const rendered =
          format === 'html' ? renderRedTeamReportHtml(report) : renderRedTeamReportMarkdown(report);
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
