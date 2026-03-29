import { readFile } from 'node:fs/promises';
import type { Command } from 'commander';
import { AirsScanService, RateLimitedScanService } from '../../airs/scanner.js';
import type { ScanService } from '../../airs/types.js';
import { loadConfig } from '../../config/loader.js';
import { computeMetrics } from '../../core/metrics.js';
import { loadPrompts } from '../../core/prompt-loader.js';
import type { TestCase, TestResult } from '../../core/types.js';
import {
  buildEvalOutput,
  type EvalOutput,
  renderError,
  renderEvalTerminal,
} from '../renderer/index.js';

export async function evalTopic(
  scanner: ScanService,
  profileName: string,
  topicName: string,
  cases: TestCase[],
  concurrency = 5,
  intent: 'block' | 'allow' = 'block',
): Promise<EvalOutput> {
  const prompts = cases.map((c) => c.prompt);
  const scanResults = await scanner.scanBatch(profileName, prompts, concurrency);

  if (scanResults.length !== cases.length) {
    throw new Error(`scanBatch returned ${scanResults.length} results for ${cases.length} prompts`);
  }

  const testResults: TestResult[] = cases.map((testCase, i) => {
    const scan = scanResults[i];
    return {
      testCase,
      actualTriggered: scan.triggered,
      scanAction: scan.action,
      scanId: scan.scanId,
      reportId: scan.reportId,
      correct: testCase.expectedTriggered === scan.triggered,
    };
  });

  const metrics = computeMetrics(testResults);
  return buildEvalOutput(profileName, topicName, intent, metrics, testResults);
}

export function registerEvalCommand(parent: Command): void {
  parent
    .command('eval')
    .description('Evaluate a topic against a static prompt set and compute metrics')
    .requiredOption('--profile <name>', 'Security profile name')
    .requiredOption('--prompts <path>', 'Path to CSV file with prompt,expected,intent columns')
    .option('--topic <name>', 'Topic name (for output labeling)', 'unknown')
    .option('--format <format>', 'Output format: json or terminal', 'terminal')
    .option('--rate <n>', 'Max AIRS scan API calls per second')
    .option('--concurrency <n>', 'Concurrent scan requests', '5')
    .action(async (opts) => {
      try {
        const config = await loadConfig();
        const csvContent = await readFile(opts.prompts, 'utf-8');
        const { cases, intent } = loadPrompts(csvContent, (msg) =>
          console.warn(`  Warning: ${msg}`),
        );

        if (!config.airsApiKey) {
          renderError('PANW_AI_SEC_API_KEY is required');
          process.exit(1);
        }
        let scanner: ScanService = new AirsScanService(config.airsApiKey);
        if (opts.rate) {
          scanner = new RateLimitedScanService(scanner, Number.parseInt(opts.rate, 10));
        }

        const concurrency = Number.parseInt(opts.concurrency, 10);
        const result = await evalTopic(
          scanner,
          opts.profile,
          opts.topic,
          cases,
          concurrency,
          intent,
        );

        if (opts.format === 'json') {
          console.log(JSON.stringify(result, null, 2));
        } else {
          renderEvalTerminal(result);
        }
      } catch (err) {
        renderError(err instanceof Error ? err.message : String(err));
        process.exit(1);
      }
    });
}
