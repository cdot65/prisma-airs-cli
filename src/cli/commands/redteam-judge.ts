import { lstat, mkdir, readFile } from 'node:fs/promises';
import { join, resolve } from 'node:path';
import { RedTeamClient } from '@cdot65/prisma-airs-sdk';
import { type Command, Option } from 'commander';
import { redTeamClientOptions } from '../../config/client-options.js';
import { settingRemedy } from '../../config/credentials.js';
import { type ConfigContext, loadConfig, resolveConfigContext } from '../../config/loader.js';
import type { Config } from '../../config/schema.js';
import {
  aggregate,
  DEFAULT_SUCCESS_THRESHOLD,
  DEFAULT_TYPESAFE_BASE_URL,
  DEFAULT_TYPESAFE_MODEL,
  DEFAULT_UNCERTAIN_BAND,
  fetchJobAttackRecords,
  type JudgeProvider,
  judgeUnits,
  normalizeScan,
  QUESTIONS,
  type RecordedJudgment,
  ReplayProvider,
  recordingFile,
  redactState,
  renderSummary,
  type SuccessPolicy,
  TypeSafeHttpProvider,
} from '../../redteam/judge/index.js';
import { writeReportFile } from '../../reports/io.js';
import { examples } from '../examples.js';
import { CliUsageError, fail, resolveOutput, ui } from '../renderer/index.js';

/** Exit code when judging finished but at least one unit hit a provider error. */
export const JUDGE_PROVIDER_ERROR_EXIT_CODE = 4;
export const JUDGE_OUTPUT_FILES = ['results.json', 'judgments.json', 'summary.md'] as const;

interface JudgeOptions {
  harnessCredentials?: boolean;
  job?: string;
  out: string;
  provider: string;
  replay?: string;
  record?: string;
  model?: string;
  baseUrl?: string;
  threshold: string;
  uncertainBand: string;
  limit?: string;
  concurrency: string;
  maxRetries: string;
  timeout: string;
  includeText?: boolean;
  dryRun?: boolean;
  output?: string;
}

function probability(raw: string, flag: string): number {
  const value = Number(raw);
  if (!/^\S+$/.test(raw) || !Number.isFinite(value) || value < 0 || value > 1)
    throw new CliUsageError(`${flag} must be a number from 0 to 1`);
  return value;
}

function integer(raw: string, flag: string, max = Number.MAX_SAFE_INTEGER): number {
  const value = Number(raw);
  if (!/^[1-9]\d*$/.test(raw) || !Number.isSafeInteger(value) || value > max)
    throw new CliUsageError(`${flag} must be an integer from 1 to ${max}`);
  return value;
}

/** Validate the policy flags; exported for tests. */
export function parsePolicy(threshold: string, uncertainBand: string): SuccessPolicy {
  const parts = uncertainBand.split(',');
  if (parts.length !== 2) throw new CliUsageError('--uncertain-band must be LOW,HIGH');
  const [low, high] = parts.map((part) => probability(part.trim(), '--uncertain-band'));
  if (low > high) throw new CliUsageError('--uncertain-band LOW must not exceed HIGH');
  return {
    threshold: probability(threshold, '--threshold'),
    uncertain_low: low,
    uncertain_high: high,
  };
}

async function readJson(path: string, what: string): Promise<unknown> {
  try {
    return JSON.parse(await readFile(path, 'utf8'));
  } catch {
    throw new CliUsageError(`${what} must name a readable JSON file`);
  }
}

async function exists(path: string): Promise<boolean> {
  try {
    await lstat(path);
    return true;
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code !== 'ENOENT') throw error;
    return false;
  }
}

function safeContext(): ConfigContext | undefined {
  try {
    return resolveConfigContext();
  } catch {
    return undefined;
  }
}

/** Register `redteam judge`: an independent ASR judged by TypeSafe Jev. */
export function registerRedTeamJudgeCommand(redteam: Command): void {
  const command = redteam
    .command('judge [scanFile]')
    .description(
      'Judge red-team attack outputs with TypeSafe Jev and compute an independent attack success rate',
    )
    .option('--job <jobId>', 'Fetch the attacks of a completed scan instead of reading a file')
    .requiredOption('--out <dir>', 'Directory for results.json, judgments.json and summary.md')
    .option('--provider <name>', 'typesafe (network) or replay (recorded answers)', 'typesafe')
    .option('--replay <file>', 'Recorded judgments file for --provider replay')
    .option('--record <file>', 'Write raw provider answers here for later replay')
    .addOption(new Option('--harness-credentials').hideHelp())
    .option('--model <id>', `TypeSafe model (default: typesafeModel or ${DEFAULT_TYPESAFE_MODEL})`)
    .option(
      '--base-url <url>',
      `TypeSafe API base URL (default: typesafeBaseUrl or ${DEFAULT_TYPESAFE_BASE_URL})`,
    )
    .option(
      '--threshold <p>',
      'P(success) at or above which an output counts as a successful attack',
      String(DEFAULT_SUCCESS_THRESHOLD),
    )
    .option(
      '--uncertain-band <low,high>',
      'P(success) range flagged for human review',
      DEFAULT_UNCERTAIN_BAND.join(','),
    )
    .option('--limit <n>', 'Judge at most N units (budget control)')
    .option('--concurrency <n>', 'Parallel judge requests (1-20)', '4')
    .option('--max-retries <n>', 'Retries per unit on 408/429/5xx', '2')
    .option('--timeout <ms>', 'Per-request deadline in milliseconds', '30000')
    .option('--include-text', 'Include prompt and response text in judgments.json', false)
    .option(
      '--dry-run',
      'Print ingestion notes, unit count, questions and a redacted example state; no judge traffic',
      false,
    )
    .option('--output <format>', 'Terminal summary: pretty (markdown) or json (results.json)')
    .addHelpText(
      'after',
      examples(
        'airs-cli redteam judge ./scan.json --out ./judged --dry-run',
        'airs-cli redteam judge ./scan.json --out ./judged --limit 25 --record ./probe.json',
        'airs-cli redteam judge --job <jobId> --out ./judged --record ./full-run-1.json',
        'airs-cli redteam judge ./scan.json --out ./judged-0.6 --provider replay --replay ./full-run-1.json --threshold 0.6',
      ),
    )
    .action(async (scanFile: string | undefined, opts: JudgeOptions) => {
      try {
        await runJudge(command, scanFile, opts);
      } catch (error) {
        fail(error);
      }
    });
}

async function runJudge(
  command: Command,
  scanFile: string | undefined,
  opts: JudgeOptions,
): Promise<void> {
  if ((scanFile === undefined) === (opts.job === undefined))
    throw new CliUsageError('Pass a scan file or --job <jobId>, not both');
  if (opts.provider !== 'typesafe' && opts.provider !== 'replay')
    throw new CliUsageError('--provider must be typesafe or replay');
  if (opts.provider === 'replay' && !opts.replay)
    throw new CliUsageError('--replay is required with --provider replay');
  if (opts.provider !== 'replay' && opts.replay)
    throw new CliUsageError('--replay requires --provider replay');
  const policy = parsePolicy(opts.threshold, opts.uncertainBand);
  const limit = opts.limit === undefined ? undefined : integer(opts.limit, '--limit');
  const concurrency = integer(opts.concurrency, '--concurrency', 20);
  const maxRetries = opts.maxRetries === '0' ? 0 : integer(opts.maxRetries, '--max-retries', 10);
  const timeoutMs = integer(opts.timeout, '--timeout');
  if (!opts.out.trim()) throw new CliUsageError('--out cannot be empty');

  // Only a live judge run or an AIRS fetch needs the tenant; replaying a file does not.
  const needsConfig =
    opts.job !== undefined ||
    (opts.provider === 'typesafe' && !opts.dryRun && !opts.harnessCredentials);
  const config: Config | undefined = needsConfig
    ? await loadConfig()
    : await loadConfig().catch(() => undefined);
  const format = await resolveOutput(command, opts, {
    allowed: ['pretty', 'json'],
    ignoreConfig: config === undefined,
  });

  const outDir = resolve(opts.out);
  const targets = JUDGE_OUTPUT_FILES.map((name) => join(outDir, name));
  const recordPath = opts.record ? resolve(opts.record) : undefined;
  if (!opts.dryRun) {
    const destinations = [...targets, ...(recordPath ? [recordPath] : [])];
    if (new Set(destinations).size !== destinations.length)
      throw new CliUsageError('Output destinations must be distinct new files');
    for (const path of [...targets, ...(recordPath ? [recordPath] : [])])
      if (await exists(path))
        throw new Error(`Output already exists: ${path}. Existing files are never overwritten.`);
  }

  let document: unknown;
  if (opts.job !== undefined) {
    if (!config) throw new Error('Tenant configuration is required for --job');
    const client = new RedTeamClient(redTeamClientOptions(config));
    ui.status(
      `Fetching attacks for job ${opts.job.replace(/[\p{Cc}\p{Cf}]/gu, '_')} (read-only)...`,
    );
    document = await fetchJobAttackRecords(client.reports, opts.job, {
      concurrency: config.scanConcurrency,
    });
  } else {
    document = await readJson(scanFile as string, 'The scan file');
  }
  let normalized: ReturnType<typeof normalizeScan>;
  try {
    normalized = normalizeScan(document, { limit });
  } catch (error) {
    throw new CliUsageError(
      `Cannot ingest scan: ${error instanceof Error ? error.message : String(error)}`,
    );
  }
  const { units, notes } = normalized;
  // The managed harness explicitly scopes these variables to this judge child.
  // Standalone commands continue to use tenant JSON and ignore credential env vars.
  const model =
    opts.model ??
    (opts.harnessCredentials ? process.env.TYPESAFE_DEFAULT_MODEL : config?.typesafeModel) ??
    DEFAULT_TYPESAFE_MODEL;
  const baseUrl =
    opts.baseUrl ??
    (opts.harnessCredentials ? process.env.TYPESAFE_BASE_URL : config?.typesafeBaseUrl) ??
    DEFAULT_TYPESAFE_BASE_URL;

  if (opts.dryRun) {
    const first = units.find((unit) => !unit.is_error);
    const payload: Record<string, unknown> = {
      ingestion: notes,
      units: units.length,
      model,
      questions: QUESTIONS,
    };
    if (first) payload.example_state = redactState(first);
    console.log(JSON.stringify(payload, null, 2));
    return;
  }

  let provider: JudgeProvider;
  if (opts.provider === 'replay') {
    const recorded = await readJson(opts.replay as string, '--replay');
    if (recorded === null || typeof recorded !== 'object' || Array.isArray(recorded))
      throw new CliUsageError('--replay must contain a JSON object');
    provider = new ReplayProvider(recorded as Record<string, unknown>);
  } else {
    const apiKey = opts.harnessCredentials ? process.env.TYPESAFE_API_KEY : config?.typesafeApiKey;
    if (typeof apiKey !== 'string' || !apiKey.trim())
      throw new Error(
        opts.harnessCredentials
          ? 'No TypeSafe key reached the judge from its AIRS environment. Check airs env typesafe status, or save a key with airs env typesafe set. No live judgment was performed; replay was not substituted.'
          : `TypeSafe API key is not configured (typesafeApiKey). ${settingRemedy(['typesafeApiKey'], safeContext())}`,
      );
    provider = new TypeSafeHttpProvider({ apiKey, model, baseUrl, maxRetries, timeoutMs });
  }

  ui.status(
    `Judging ${units.length} unit(s) with ${provider.name} (model ${provider.model}, concurrency ${concurrency})...`,
  );
  const recorder: Record<string, RecordedJudgment> | undefined = recordPath ? {} : undefined;
  const judgments = await judgeUnits(units, provider, policy, { concurrency, recorder });
  const results = aggregate(judgments, policy, {
    providerName: provider.name,
    model: provider.model,
    ingestion: notes,
  });
  const rows: Array<Record<string, unknown>> = judgments.map((judgment) => ({ ...judgment }));
  if (opts.includeText) {
    const byId = new Map(units.map((unit) => [unit.unit_id, unit]));
    for (const row of rows) {
      const unit = byId.get(row.unit_id as string);
      row.prompt = unit?.prompt;
      row.response_text = unit?.response_text;
    }
  }
  const summary = renderSummary(results);

  await mkdir(outDir, { recursive: true, mode: 0o700 });
  const documents: Array<[string, string]> = [
    [targets[0], `${JSON.stringify(results, null, 2)}\n`],
    [targets[1], `${JSON.stringify(rows, null, 2)}\n`],
    [targets[2], summary],
  ];
  if (recorder && recordPath)
    documents.push([
      recordPath,
      `${JSON.stringify(recordingFile(results.model, QUESTIONS, recorder), null, 2)}\n`,
    ]);
  for (const [path, content] of documents) await writeReportFile(path, content);
  ui.status(`Judgment → ${outDir.replace(/[\p{Cc}\p{Cf}]/gu, '_')} (mode 0600)`);

  if (format === 'json') console.log(JSON.stringify(results, null, 2));
  else console.log(summary);
  if (results.coverage.provider_error > 0) {
    ui.error(
      `${results.coverage.provider_error} unit(s) hit provider errors; see judgments.json (status provider_error)`,
    );
    process.exitCode = JUDGE_PROVIDER_ERROR_EXIT_CODE;
  }
}
