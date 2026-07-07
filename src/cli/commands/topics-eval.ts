import { readFile } from 'node:fs/promises';
import type { Command } from 'commander';
import { AirsScanService, RateLimitedScanService } from '../../airs/scanner.js';
import type { ScanService } from '../../airs/types.js';
import { runtimeInitOptions } from '../../config/client-options.js';
import { loadConfig } from '../../config/loader.js';
import { computeMetrics } from '../../core/metrics.js';
import { loadPrompts } from '../../core/prompt-loader.js';
import type { TestCase, TestResult } from '../../core/types.js';
import { registerDeprecatedAlias, resolveDeprecatedAliases } from '../deprecated-flags.js';
import { examples } from '../examples.js';
import {
  buildEvalOutput,
  type EvalOutput,
  fail,
  renderEvalTerminal,
  ui,
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
  const cmd = parent
    .command('eval')
    .description('Evaluate a topic against a static prompt set and compute metrics')
    .requiredOption('--profile <name>', 'Security profile name')
    .requiredOption('--prompts <path>', 'Path to CSV file with prompt,expected,intent columns')
    .option('--topic <name>', 'Topic name (for output labeling)', 'unknown')
    .option('--output <format>', 'Output format: pretty or json', 'pretty')
    .option('--rate <n>', 'Max AIRS scan API calls per second')
    .option('--concurrency <n>', 'Concurrent scan requests', '5')
    .addHelpText(
      'after',
      examples(
        'airs runtime topics eval --profile prod-guard --prompts eval.csv --topic "Financial Advice"',
        'airs runtime topics eval --profile prod-guard --prompts eval.csv --output json',
        'airs runtime topics eval --profile prod-guard --prompts eval.csv --rate 5 --concurrency 3',
      ),
    );
  registerDeprecatedAlias(cmd, {
    oldFlag: '--format <format>',
    oldKey: 'format',
    canonicalFlag: '--output',
    canonicalKey: 'output',
  });
  cmd.action(async (opts) => {
    resolveDeprecatedAliases(cmd, opts);
    try {
      const config = await loadConfig();
      const csvContent = await readFile(opts.prompts, 'utf-8');
      const { cases, intent } = loadPrompts(csvContent, (msg) => ui.status(`Warning: ${msg}`));

      if (!config.airsApiKey && !config.airsApiToken) {
        fail(new Error('PANW_AI_SEC_API_KEY or PANW_AI_SEC_API_TOKEN is required'));
      }
      let scanner: ScanService = new AirsScanService(runtimeInitOptions(config));
      if (opts.rate) {
        scanner = new RateLimitedScanService(scanner, Number.parseInt(opts.rate, 10));
      }

      const concurrency = Number.parseInt(opts.concurrency, 10);
      const result = await evalTopic(scanner, opts.profile, opts.topic, cases, concurrency, intent);

      if (opts.output === 'json') {
        console.log(JSON.stringify(result, null, 2));
      } else {
        renderEvalTerminal(result);
      }
    } catch (err) {
      fail(err);
    }
  });
}
