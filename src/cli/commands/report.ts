import { writeFile } from 'node:fs/promises';
import chalk from 'chalk';
import type { Command } from 'commander';
import { loadConfig } from '../../config/loader.js';
import { JsonFileStore } from '../../persistence/store.js';
import { buildReportHtml } from '../../report/html.js';
import { buildReportJson } from '../../report/json.js';
import {
  renderAnalysis,
  renderError,
  renderHeader,
  renderMetrics,
  renderTestResults,
  renderTopic,
} from '../renderer/index.js';

/** Register the `report` command — view detailed results for a run. */
export function registerReportCommand(parent: Command): void {
  parent
    .command('report <runId>')
    .description('View detailed report for a run')
    .option('--iteration <n>', 'Show specific iteration')
    .option('--format <format>', 'Output format: terminal, json, html', 'terminal')
    .option('--tests', 'Include per-test-case details')
    .option('--diff <runId>', 'Compare with another run')
    .option('--output <path>', 'Output file path (html format)')
    .action(async (runId: string, opts) => {
      try {
        const config = await loadConfig();
        const store = new JsonFileStore(config.dataDir);
        const run = await store.load(runId);

        if (!run) {
          renderError(`Run ${runId} not found`);
          process.exit(1);
        }

        let diffRun: Awaited<ReturnType<typeof store.load>> = null;
        if (opts.diff) {
          diffRun = await store.load(opts.diff);
          if (!diffRun) {
            renderError(`Diff run ${opts.diff} not found`);
            process.exit(1);
          }
        }

        const format: string = opts.format;

        if (format === 'json') {
          const report = buildReportJson(run, {
            includeTests: opts.tests ?? false,
            diffRun,
          });
          console.log(JSON.stringify(report, null, 2));
          return;
        }

        if (format === 'html') {
          const report = buildReportJson(run, {
            includeTests: opts.tests ?? false,
            diffRun,
          });
          const html = buildReportHtml(report);
          const outputPath = opts.output ?? `${runId}-report.html`;
          await writeFile(outputPath, html, 'utf-8');
          console.log(chalk.green(`  Report written to ${outputPath}`));
          return;
        }

        // Terminal format (default — unchanged behavior)
        renderHeader();
        console.log(chalk.bold(`\n  Run: ${run.id}`));
        console.log(`  Status: ${run.status}`);
        console.log(`  Created: ${run.createdAt}`);
        console.log(`  Topic: ${run.userInput.topicDescription}`);
        console.log(`  Intent: ${run.userInput.intent}`);
        console.log(
          `  Best coverage: ${(run.bestCoverage * 100).toFixed(1)}% (iteration ${run.bestIteration})`,
        );
        console.log(`  Total iterations: ${run.iterations.length}`);

        if (opts.iteration) {
          const idx = Number.parseInt(opts.iteration, 10) - 1;
          const iter = run.iterations[idx];
          if (!iter) {
            renderError(`Iteration ${opts.iteration} not found`);
            process.exit(1);
          }
          console.log(chalk.bold(`\n  Iteration ${iter.iteration}:`));
          renderTopic(iter.topic);
          renderMetrics(iter.metrics);
          renderAnalysis(iter.analysis);
          if (opts.tests) renderTestResults(iter.testResults);
        } else {
          // Show best iteration
          const best = run.iterations[run.bestIteration - 1];
          if (best) {
            console.log(chalk.bold(`\n  Best Iteration (${best.iteration}):`));
            renderTopic(best.topic);
            renderMetrics(best.metrics);
            renderAnalysis(best.analysis);
            if (opts.tests) renderTestResults(best.testResults);
          }
        }

        if (opts.diff && diffRun) {
          const report = buildReportJson(run, { diffRun });
          if (report.diff) {
            const d = report.diff;
            console.log(chalk.bold('\n  Run Comparison:'));
            console.log(`  Base: ${d.baseRunId} | Compare: ${d.compareRunId}`);
            const fmtDelta = (label: string, val: number) => {
              const sign = val >= 0 ? '+' : '';
              const color = val > 0 ? chalk.green : val < 0 ? chalk.red : chalk.gray;
              console.log(`  ${label}: ${color(`${sign}${(val * 100).toFixed(1)}%`)}`);
            };
            fmtDelta('Coverage', d.metricsDelta.coverage);
            fmtDelta('TPR', d.metricsDelta.tpr);
            fmtDelta('TNR', d.metricsDelta.tnr);
            fmtDelta('Accuracy', d.metricsDelta.accuracy);
            fmtDelta('F1', d.metricsDelta.f1);
          }
        }

        console.log();
      } catch (err) {
        renderError(err instanceof Error ? err.message : String(err));
        process.exit(1);
      }
    });
}
