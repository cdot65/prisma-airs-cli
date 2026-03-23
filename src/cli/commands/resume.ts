import * as path from 'node:path';
import type { Command } from 'commander';
import { SdkManagementService } from '../../airs/management.js';
import { SdkPromptSetService } from '../../airs/promptsets.js';
import { AirsScanService, DebugScanService, RateLimitedScanService } from '../../airs/scanner.js';
import type { ScanService } from '../../airs/types.js';
import { loadConfig } from '../../config/loader.js';
import { runLoop } from '../../core/loop.js';
import { createLlmProvider } from '../../llm/provider.js';
import { LangChainLlmService } from '../../llm/service.js';
import { JsonFileStore } from '../../persistence/store.js';
import {
  renderAnalysis,
  renderCompanionTopic,
  renderError,
  renderHeader,
  renderIterationStart,
  renderIterationSummary,
  renderLoopComplete,
  renderMetrics,
  renderTestProgress,
  renderTestsAccumulated,
  renderTestsComposed,
  renderTopic,
} from '../renderer/index.js';

/** Register the `resume` command — resumes a paused or failed run. */
export function registerResumeCommand(parent: Command): void {
  parent
    .command('resume <runId>')
    .description('Resume a paused or failed run')
    .option('--max-iterations <n>', 'Additional iterations to run', '10')
    .option('--rate <n>', 'Max AIRS scan API calls per second (default: unlimited)')
    .option('--debug-scans', 'Dump raw AIRS scan responses to JSONL for debugging', false)
    .option('--create-prompt-set', 'Create custom prompt set from test cases after loop', false)
    .option('--prompt-set-name <name>', 'Override auto-generated prompt set name')
    .action(async (runId: string, opts) => {
      try {
        renderHeader();
        const config = await loadConfig();
        const store = new JsonFileStore(config.dataDir);
        const existingRun = await store.load(runId);

        if (!existingRun) {
          renderError(`Run ${runId} not found`);
          process.exit(1);
        }

        if (existingRun.status === 'completed') {
          renderError('Run is already completed');
          process.exit(1);
        }

        const additionalIterations = Number.parseInt(opts.maxIterations, 10);

        // Resume with updated max iterations
        const userInput = {
          ...existingRun.userInput,
          maxIterations: existingRun.currentIteration + additionalIterations,
          createPromptSet: opts.createPromptSet ?? false,
          promptSetName: opts.promptSetName,
        };

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
        if (!config.airsApiKey) throw new Error('PANW_AI_SEC_API_KEY is required');
        let scanner: ScanService = new AirsScanService(config.airsApiKey);
        if (opts.rate) {
          scanner = new RateLimitedScanService(scanner, Number.parseInt(opts.rate, 10));
        }
        if (opts.debugScans) {
          const debugPath = path.join(config.dataDir, '..', `debug-scans-${runId}.jsonl`);
          scanner = new DebugScanService(scanner, debugPath);
        }
        const management = new SdkManagementService({
          clientId: config.mgmtClientId,
          clientSecret: config.mgmtClientSecret,
          tsgId: config.mgmtTsgId,
          apiEndpoint: config.mgmtEndpoint,
          tokenEndpoint: config.mgmtTokenEndpoint,
        });

        // Set up prompt set service if requested
        const promptSets = userInput.createPromptSet
          ? new SdkPromptSetService({
              clientId: config.mgmtClientId,
              clientSecret: config.mgmtClientSecret,
              tsgId: config.mgmtTsgId,
              tokenEndpoint: config.mgmtTokenEndpoint,
            })
          : undefined;

        console.log(`  Resuming run ${runId} from iteration ${existingRun.currentIteration}...`);

        for await (const event of runLoop(userInput, {
          llm,
          management,
          scanner,
          promptSets,
        })) {
          switch (event.type) {
            case 'iteration:start':
              renderIterationStart(event.iteration);
              break;
            case 'generate:complete':
              renderTopic(event.topic);
              break;
            case 'companion:generated':
              renderCompanionTopic(event.topic);
              break;
            case 'companion:created':
              console.log(`  \u2713 Companion allow topic created (${event.topicId})`);
              break;
            case 'tests:composed':
              renderTestsComposed(
                event.generated,
                event.carriedFailures,
                event.regressionTier,
                event.total,
              );
              break;
            case 'tests:accumulated':
              renderTestsAccumulated(event.newCount, event.totalCount, event.droppedCount);
              break;
            case 'test:progress':
              renderTestProgress(event.completed, event.total);
              break;
            case 'evaluate:complete':
              renderMetrics(event.metrics);
              break;
            case 'analyze:complete':
              renderAnalysis(event.analysis);
              break;
            case 'iteration:complete':
              renderIterationSummary(event.result);
              break;
            case 'promptset:created':
              console.log(
                `  ✓ Custom prompt set created: ${event.promptSetName} (${event.promptCount} prompts)`,
              );
              break;
            case 'loop:complete':
              await store.save(event.runState);
              renderLoopComplete(event.runState);
              break;
          }
        }
      } catch (err) {
        renderError(err instanceof Error ? err.message : String(err));
        process.exit(1);
      }
    });
}
