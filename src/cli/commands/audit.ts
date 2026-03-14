import { writeFile } from 'node:fs/promises';
import chalk from 'chalk';
import type { Command } from 'commander';
import { SdkManagementService } from '../../airs/management.js';
import { AirsScanService } from '../../airs/scanner.js';
import { buildAuditReportHtml, buildAuditReportJson } from '../../audit/report.js';
import { runAudit } from '../../audit/runner.js';
import { loadConfig } from '../../config/loader.js';
import { createLlmProvider } from '../../llm/provider.js';
import { LangChainLlmService } from '../../llm/service.js';
import {
  renderAuditComplete,
  renderAuditTopics,
  renderConflicts,
  renderError,
  renderMetrics,
} from '../renderer/index.js';

/** Register the `audit` command — evaluate all topics in a profile. */
export function registerAuditCommand(parent: Command): void {
  parent
    .command('audit <profileName>')
    .description('Evaluate all topics in a security profile')
    .option('--max-tests-per-topic <n>', 'Max tests per topic', '20')
    .option('--format <format>', 'Output format: terminal, json, html', 'terminal')
    .option('--output <path>', 'Output file path (json/html)')
    .option('--provider <provider>', 'LLM provider override')
    .option('--model <model>', 'LLM model override')
    .action(async (profileName: string, opts) => {
      try {
        const config = await loadConfig({
          llmProvider: opts.provider,
          llmModel: opts.model,
        });

        const model = await createLlmProvider({
          provider: config.llmProvider,
          model: config.llmModel,
          anthropicApiKey: config.anthropicApiKey,
          googleApiKey: config.googleApiKey,
          googleCloudProject: config.googleCloudProject,
          googleCloudLocation: config.googleCloudLocation,
          awsRegion: config.awsRegion,
          awsAccessKeyId: config.awsAccessKeyId,
          awsSecretAccessKey: config.awsSecretAccessKey,
        });
        const llm = new LangChainLlmService(model);

        const management = new SdkManagementService({
          clientId: config.mgmtClientId,
          clientSecret: config.mgmtClientSecret,
          tsgId: config.mgmtTsgId,
          tokenEndpoint: config.mgmtTokenEndpoint,
        });

        if (!config.airsApiKey) {
          renderError('PANW_AI_SEC_API_KEY is required');
          process.exit(1);
        }
        const scanner = new AirsScanService(config.airsApiKey);

        const format: string = opts.format;

        console.log(chalk.bold.cyan('\n  Prisma AIRS Profile Audit'));
        console.log(chalk.dim(`  Evaluating all topics in "${profileName}"\n`));

        for await (const event of runAudit(
          {
            profileName,
            maxTestsPerTopic: Number.parseInt(opts.maxTestsPerTopic, 10),
            scanConcurrency: config.scanConcurrency,
          },
          { llm, management, scanner },
        )) {
          switch (event.type) {
            case 'topics:loaded':
              console.log(chalk.dim(`  Found ${event.topics.length} topic(s) in profile\n`));
              renderAuditTopics(event.topics);
              break;
            case 'tests:generated':
              console.log(chalk.dim(`  Generated ${event.count} tests for "${event.topicName}"`));
              break;
            case 'scan:progress':
              console.log(chalk.dim(`\n  Scanned ${event.completed}/${event.total} prompts`));
              break;
            case 'evaluate:complete':
              break;
            case 'audit:complete': {
              const { result } = event;

              if (format === 'json') {
                const report = buildAuditReportJson(result);
                const output = JSON.stringify(report, null, 2);
                if (opts.output) {
                  await writeFile(opts.output, output, 'utf-8');
                  console.log(chalk.green(`\n  Report written to ${opts.output}`));
                } else {
                  console.log(output);
                }
                return;
              }

              if (format === 'html') {
                const html = buildAuditReportHtml(result);
                const outputPath = opts.output ?? `audit-${profileName.replace(/\s+/g, '-')}.html`;
                await writeFile(outputPath, html, 'utf-8');
                console.log(chalk.green(`\n  Report written to ${outputPath}`));
                return;
              }

              // Terminal format
              renderAuditComplete(result);
              console.log(chalk.bold('\n  Composite Metrics:'));
              renderMetrics(result.compositeMetrics);
              if (result.conflicts.length > 0) {
                renderConflicts(result.conflicts);
              } else {
                console.log(chalk.green('\n  No cross-topic conflicts detected.\n'));
              }
              break;
            }
          }
        }
      } catch (err) {
        renderError(err instanceof Error ? err.message : String(err));
        process.exit(1);
      }
    });
}
