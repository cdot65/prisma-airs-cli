import * as fs from 'node:fs';
import * as path from 'node:path';
import type { Command } from 'commander';
import { SdkManagementService } from '../../airs/management.js';
import { SdkPromptSetService } from '../../airs/promptsets.js';
import { AirsScanService, DebugScanService } from '../../airs/scanner.js';
import type { ScanService } from '../../airs/types.js';
import { loadConfig } from '../../config/loader.js';
import { runLoop } from '../../core/loop.js';
import type { UserInput } from '../../core/types.js';
import { createLlmProvider } from '../../llm/provider.js';
import { LangChainLlmService } from '../../llm/service.js';
import { LearningExtractor } from '../../memory/extractor.js';
import { MemoryInjector } from '../../memory/injector.js';
import { MemoryStore } from '../../memory/store.js';
import { JsonFileStore } from '../../persistence/store.js';
import { collectUserInput } from '../prompts.js';
import {
  renderAnalysis,
  renderCompanionTopic,
  renderError,
  renderHeader,
  renderIterationStart,
  renderIterationSummary,
  renderLoopComplete,
  renderMemoryExtracted,
  renderMemoryLoaded,
  renderMetrics,
  renderTestProgress,
  renderTestsAccumulated,
  renderTestsComposed,
  renderTopic,
} from '../renderer/index.js';

/** Register the `generate` command — starts a new guardrail generation loop. */
export function registerGenerateCommand(parent: Command): void {
  parent
    .command('generate')
    .description('Start a new guardrail generation loop')
    .option('--provider <provider>', 'LLM provider')
    .option('--model <model>', 'LLM model name')
    .option('--profile <name>', 'AIRS security profile name')
    .option('--topic <description>', 'Topic description')
    .option('--intent <intent>', 'Intent: block or allow')
    .option('--max-iterations <n>', 'Max iterations', '20')
    .option('--target-coverage <n>', 'Target coverage %', '90')
    .option(
      '--max-regressions <n>',
      'Stop after N consecutive coverage regressions (0 = disable)',
      '3',
    )
    .option('--plateau-window <n>', 'Iterations to check for plateau (0 = disable)', '0')
    .option('--plateau-band <pct>', 'Max coverage variance for plateau detection', '0.05')
    .option('--accumulate-tests', 'Carry forward test prompts across iterations', false)
    .option('--max-accumulated-tests <n>', 'Max accumulated test count cap')
    .option('--memory', 'Enable learning memory (default)')
    .option('--no-memory', 'Disable learning memory')
    .option('--debug-scans', 'Dump raw AIRS scan responses to JSONL for debugging', false)
    .option('--create-prompt-set', 'Create custom prompt set from test cases after loop', false)
    .option('--prompt-set-name <name>', 'Override auto-generated prompt set name')
    .option('--save-tests <path>', 'Save best iteration test cases to CSV')
    .action(async (opts) => {
      try {
        renderHeader();

        const config = await loadConfig({
          llmProvider: opts.provider,
          llmModel: opts.model,
          memoryEnabled: opts.memory !== undefined ? String(opts.memory) : undefined,
        });

        // Collect user input (interactive or from CLI flags)
        let userInput: UserInput;
        if (opts.topic && opts.profile) {
          userInput = {
            topicDescription: opts.topic,
            intent: (opts.intent ?? 'block') as 'block' | 'allow',
            profileName: opts.profile,
            maxIterations: Number.parseInt(opts.maxIterations, 10),
            targetCoverage: Number.parseInt(opts.targetCoverage, 10) / 100,
            maxRegressions: Number.parseInt(opts.maxRegressions, 10),
            plateauWindow: Number.parseInt(opts.plateauWindow, 10),
            plateauBand: Number.parseFloat(opts.plateauBand),
            accumulateTests: opts.accumulateTests ?? false,
            maxAccumulatedTests: opts.maxAccumulatedTests
              ? Number.parseInt(opts.maxAccumulatedTests, 10)
              : undefined,
            createPromptSet: opts.createPromptSet ?? false,
            promptSetName: opts.promptSetName,
          };
        } else {
          userInput = await collectUserInput();
        }

        // Initialize services
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

        // Set up memory system
        const memoryEnabled = config.memoryEnabled;
        const memoryStore = memoryEnabled ? new MemoryStore(config.memoryDir) : undefined;
        const memoryInjector = memoryStore
          ? new MemoryInjector(memoryStore, config.maxMemoryChars)
          : undefined;

        const llm = new LangChainLlmService(model, memoryInjector);
        if (!config.airsApiKey) throw new Error('PANW_AI_SEC_API_KEY is required');
        let scanner: ScanService = new AirsScanService(config.airsApiKey);
        if (opts.debugScans) {
          const debugPath = path.join(config.dataDir, '..', `debug-scans-${Date.now()}.jsonl`);
          scanner = new DebugScanService(scanner, debugPath);
        }
        const management = new SdkManagementService({
          clientId: config.mgmtClientId,
          clientSecret: config.mgmtClientSecret,
          tsgId: config.mgmtTsgId,
          apiEndpoint: config.mgmtEndpoint,
          tokenEndpoint: config.mgmtTokenEndpoint,
        });

        const store = new JsonFileStore(config.dataDir);

        // Load memory before loop
        if (memoryEnabled) {
          const learningCount = await llm.loadMemory(userInput.topicDescription);
          renderMemoryLoaded(learningCount);
        }

        const memoryExtractor = memoryStore ? new LearningExtractor(model, memoryStore) : undefined;

        // Set up prompt set service if requested
        const promptSets = userInput.createPromptSet
          ? new SdkPromptSetService({
              clientId: config.mgmtClientId,
              clientSecret: config.mgmtClientSecret,
              tsgId: config.mgmtTsgId,
              tokenEndpoint: config.mgmtTokenEndpoint,
            })
          : undefined;

        // Warn about allow-intent coverage expectations
        if (userInput.intent === 'allow') {
          console.log(
            '  \u26A0 Allow-intent topics typically achieve 40\u201370% coverage due to AIRS semantic matching.',
          );
          console.log('    Consider using --target-coverage 65 for realistic expectations.');
          console.log();
        }

        // Run the loop
        for await (const event of runLoop(userInput, {
          llm,
          management,
          scanner,
          propagationDelayMs: config.propagationDelayMs,
          memory: memoryExtractor ? { extractor: memoryExtractor } : undefined,
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
            case 'topic:reverted':
              console.log(
                `  ↩ Reverted to best definition (iteration ${event.revertedToIteration})`,
              );
              renderTopic(event.topic);
              break;
            case 'topic:simplified':
              console.log('  ⚡ Topic simplified after consecutive regressions');
              renderTopic(event.topic);
              break;
            case 'probe:waiting':
              console.log(
                `  ⏳ Waiting for topic propagation (attempt ${event.attempt}/${event.maxAttempts})...`,
              );
              break;
            case 'probe:ready':
              console.log(`  ✓ Topic active after ${event.attempts} probe(s)`);
              break;
            case 'topic:duplicate':
              console.log(
                `  ⚠ Topic identical to iteration ${event.duplicateOfIteration} — skipping scan`,
              );
              break;
            case 'loop:plateau':
              console.log(
                `  ⚠ Coverage plateaued at ${(event.band[0] * 100).toFixed(1)}–${(event.band[1] * 100).toFixed(1)}%`,
              );
              console.log(
                `    Platform ceiling likely reached (best: ${(event.bestCoverage * 100).toFixed(1)}%)`,
              );
              break;
            case 'memory:extracted':
              renderMemoryExtracted(event.learningCount);
              break;
            case 'promptset:created':
              console.log(
                `  ✓ Custom prompt set created: ${event.promptSetName} (${event.promptCount} prompts)`,
              );
              break;
            case 'loop:complete':
              await store.save(event.runState);
              renderLoopComplete(event.runState);
              if (opts.saveTests) {
                const bestIdx =
                  event.runState.bestIteration > 0
                    ? event.runState.bestIteration - 1
                    : event.runState.iterations.length - 1;
                const best = event.runState.iterations[bestIdx];
                if (best) {
                  const resultMap = new Map(best.testResults.map((r) => [r.testCase.prompt, r]));
                  const csvRows = [
                    'prompt,expected_triggered,category,source,actual_triggered,correct',
                  ];
                  for (const tc of best.testCases) {
                    const tr = resultMap.get(tc.prompt);
                    const csvEscape = (s: string) => `"${s.replace(/"/g, '""')}"`;
                    csvRows.push(
                      [
                        csvEscape(tc.prompt),
                        tc.expectedTriggered,
                        csvEscape(tc.category),
                        tc.source ?? 'generated',
                        tr?.actualTriggered ?? '',
                        tr?.correct ?? '',
                      ].join(','),
                    );
                  }
                  fs.writeFileSync(opts.saveTests, csvRows.join('\n'), 'utf-8');
                  console.log(
                    `  ✓ Test cases saved to ${opts.saveTests} (${best.testCases.length} rows)`,
                  );
                }
              }
              break;
          }
        }
      } catch (err) {
        renderError(err instanceof Error ? err.message : String(err));
        process.exit(1);
      }
    });
}
