import type { FileHandle } from 'node:fs/promises';
import { open, readFile, unlink } from 'node:fs/promises';
import { AIGatewayClient } from '@cdot65/prisma-airs-sdk';
import type { Command } from 'commander';
import { dump, load } from 'js-yaml';
import { aiGatewayGrantHint } from '../../../airs/aigateway.js';
import { aiGatewayClientOptions } from '../../../config/client-options.js';
import { loadConfig } from '../../../config/loader.js';
import { confirmOrAbort } from '../../confirm.js';
import {
  CliUsageError,
  fail,
  formatOutput,
  type OutputFormat,
  resolveOutput,
  ui,
} from '../../renderer/index.js';

export type GatewayRecord = Record<string, unknown>;

type AiGatewayClientFactory = () => Promise<AIGatewayClient>;

async function defaultClientFactory(): Promise<AIGatewayClient> {
  const config = await loadConfig();
  return new AIGatewayClient(aiGatewayClientOptions(config));
}

let clientFactory: AiGatewayClientFactory = defaultClientFactory;

/** Construct lazily so help and completion never load credentials or contact SCM. */
export async function createAiGatewayClient(): Promise<AIGatewayClient> {
  return clientFactory();
}

/** Override the lazy client factory for an isolated test and return a restore callback. */
export function setAiGatewayClientFactoryForTest(factory: AiGatewayClientFactory): () => void {
  const previous = clientFactory;
  clientFactory = factory;
  return () => {
    clientFactory = previous;
  };
}

/** Render the current group help when a resource group is invoked without an action. */
export function showHelpOnEmpty(command: Command): Command {
  return command.action(() => command.outputHelp());
}

export function addReadOutput(command: Command): Command {
  return command.option(
    '--output <format>',
    'Output format: pretty, table, markdown, csv, json, yaml',
  );
}

export function addWriteOutput(command: Command): Command {
  return command.option('--output <format>', 'Output format: pretty, json, yaml');
}

export async function readGatewayRequest(path: string): Promise<GatewayRecord> {
  let parsed: unknown;
  try {
    const text = await readFile(path, 'utf8');
    parsed = path.endsWith('.yaml') || path.endsWith('.yml') ? load(text) : JSON.parse(text);
  } catch (error) {
    throw new CliUsageError(
      `Unable to parse --file '${path}' as JSON or YAML: ${error instanceof Error ? error.message : String(error)}`,
    );
  }
  if (parsed === null || typeof parsed !== 'object' || Array.isArray(parsed)) {
    throw new CliUsageError(`--file '${path}' must contain an object`);
  }
  return parsed as GatewayRecord;
}

export function failAiGateway(error: unknown): never {
  const hint = aiGatewayGrantHint(error);
  if (hint) ui.warn(`403: ${hint}`);
  fail(error);
}

export function responseItems(value: unknown): GatewayRecord[] {
  const record = asRecord(value);
  if (Array.isArray(value)) return value.map(asRecord);
  if (Array.isArray(record.data)) return record.data.map(asRecord);
  const data = asRecord(record.data);
  if (Array.isArray(data.records)) return data.records.map(asRecord);
  if (Array.isArray(record.records)) return record.records.map(asRecord);
  return [];
}

export function asRecord(value: unknown): GatewayRecord {
  return value !== null && typeof value === 'object' ? (value as GatewayRecord) : { value };
}

function displayColumns(items: GatewayRecord[]): Array<{ key: string; label: string }> {
  const preferred = [
    'id',
    'slug',
    'name',
    'type',
    'status',
    'enabled',
    'created_at',
    'last_updated_at',
  ];
  const keys = new Set(items.flatMap((item) => Object.keys(item)));
  const scalar = (key: string) =>
    items.every((item) => item[key] == null || typeof item[key] !== 'object');
  const chosen = [
    ...preferred.filter((key) => keys.has(key) && scalar(key)),
    ...[...keys].filter((key) => !preferred.includes(key) && scalar(key)).sort(),
  ].slice(0, 8);
  return chosen.map((key) => ({
    key,
    label: key.replaceAll('_', ' ').replace(/\b\w/g, (c) => c.toUpperCase()),
  }));
}

export function renderGatewayList(
  items: GatewayRecord[],
  format: OutputFormat,
  label: string,
): void {
  if (format === 'json') {
    console.log(JSON.stringify(items, null, 2));
    return;
  }
  if (format === 'yaml') {
    console.log(dump(items, { noRefs: true, lineWidth: -1 }).trimEnd());
    return;
  }
  if (items.length === 0) {
    ui.emptyList(label);
    return;
  }
  const columns = displayColumns(items);
  const rendered = formatOutput(items, columns, format === 'pretty' ? 'table' : format);
  if (rendered) console.log(rendered);
}

export function renderGatewayDetail(value: unknown, format: OutputFormat): void {
  const item = asRecord(value);
  if (format === 'json') {
    console.log(JSON.stringify(item, null, 2));
    return;
  }
  if (format === 'yaml') {
    console.log(dump(item, { noRefs: true, lineWidth: -1 }).trimEnd());
    return;
  }
  const rows = Object.entries(item).map(([key, raw]) => ({
    key,
    value: raw !== null && typeof raw === 'object' ? JSON.stringify(raw) : raw,
  }));
  const rendered = formatOutput(
    rows,
    [
      { key: 'key', label: 'Field' },
      { key: 'value', label: 'Value' },
    ],
    format === 'pretty' ? 'table' : format,
  );
  if (rendered) console.log(rendered);
}

export async function runList(
  command: Command,
  opts: { output?: string },
  label: string,
  load: (client: AIGatewayClient) => Promise<unknown>,
): Promise<void> {
  try {
    const format = await resolveOutput(command, opts);
    const client = await createAiGatewayClient();
    renderGatewayList(responseItems(await load(client)), format, label);
  } catch (error) {
    failAiGateway(error);
  }
}

export async function runDetail(
  command: Command,
  opts: { output?: string },
  load: (client: AIGatewayClient) => Promise<unknown>,
): Promise<void> {
  try {
    const format = await resolveOutput(command, opts);
    const client = await createAiGatewayClient();
    renderGatewayDetail(await load(client), format);
  } catch (error) {
    failAiGateway(error);
  }
}

export async function runWrite(
  command: Command,
  opts: { output?: string },
  write: (client: AIGatewayClient) => Promise<unknown>,
): Promise<void>;
export async function runWrite<T>(
  command: Command,
  opts: { output?: string },
  prepare: () => Promise<T>,
  write: (client: AIGatewayClient, prepared: T) => Promise<unknown>,
): Promise<void>;
export async function runWrite<T>(
  command: Command,
  opts: { output?: string },
  prepareOrWrite: (() => Promise<T>) | ((client: AIGatewayClient) => Promise<unknown>),
  preparedWrite?: (client: AIGatewayClient, prepared: T) => Promise<unknown>,
): Promise<void> {
  try {
    const format = await resolveOutput(command, opts, { allowed: ['pretty', 'json', 'yaml'] });
    const prepared = preparedWrite ? await (prepareOrWrite as () => Promise<T>)() : undefined;
    const client = await createAiGatewayClient();
    const result = preparedWrite
      ? await preparedWrite(client, prepared as T)
      : await (prepareOrWrite as (client: AIGatewayClient) => Promise<unknown>)(client);
    renderGatewayDetail(result, format);
  } catch (error) {
    failAiGateway(error);
  }
}

export async function runConfirmedWrite(
  command: Command,
  opts: { force?: boolean; output?: string },
  prompt: string,
  write: (client: AIGatewayClient) => Promise<unknown>,
): Promise<void>;
export async function runConfirmedWrite<T>(
  command: Command,
  opts: { force?: boolean; output?: string },
  prompt: string,
  prepare: () => Promise<T>,
  write: (client: AIGatewayClient, prepared: T) => Promise<unknown>,
): Promise<void>;
export async function runConfirmedWrite<T>(
  command: Command,
  opts: { force?: boolean; output?: string },
  prompt: string,
  prepareOrWrite: (() => Promise<T>) | ((client: AIGatewayClient) => Promise<unknown>),
  preparedWrite?: (client: AIGatewayClient, prepared: T) => Promise<unknown>,
): Promise<void> {
  try {
    const format = await resolveOutput(command, opts, { allowed: ['pretty', 'json', 'yaml'] });
    const prepared = preparedWrite ? await (prepareOrWrite as () => Promise<T>)() : undefined;
    const action = prompt.replace(/\?$/, '').replace(/^./, (character) => character.toLowerCase());
    await confirmOrAbort(prompt, Boolean(opts.force), { action });
    const client = await createAiGatewayClient();
    const result = preparedWrite
      ? await preparedWrite(client, prepared as T)
      : await (prepareOrWrite as (client: AIGatewayClient) => Promise<unknown>)(client);
    renderGatewayDetail(result ?? { success: true }, format);
  } catch (error) {
    failAiGateway(error);
  }
}

interface SecretOutputOptions {
  force?: boolean;
  output?: string;
  secretOutput?: string;
  showSecret?: boolean;
}

interface SecretWriteSettings<T> {
  /** Whether this particular request can return a one-time credential. */
  requiresDestination?: (prepared: T) => boolean;
  /** Redact an ordinary, non-secret-bearing response before rendering. */
  redactResponse?: (result: unknown) => unknown;
}

/** Run a one-time-secret mutation only after its destination is explicit. */
export async function runSecretWrite<T>(
  command: Command,
  opts: SecretOutputOptions,
  prepare: () => Promise<T>,
  write: (client: AIGatewayClient, prepared: T) => Promise<unknown>,
  prompt?: string,
  settings: SecretWriteSettings<T> = {},
): Promise<void> {
  let destinationHandle: FileHandle | undefined;
  let destinationReserved = false;
  let secretPersisted = false;
  const cleanupReservedDestination = async () => {
    if (destinationHandle) await destinationHandle.close().catch(() => undefined);
    destinationHandle = undefined;
    if (opts.secretOutput && destinationReserved && !secretPersisted) {
      await unlink(opts.secretOutput).catch(() => undefined);
    }
    destinationReserved = false;
  };
  try {
    const prepared = await prepare();
    const requiresDestination = settings.requiresDestination?.(prepared) ?? true;
    if (requiresDestination && !opts.secretOutput && !opts.showSecret) {
      throw new CliUsageError(
        'Choose --secret-output <path> (recommended) or --show-secret before requesting a one-time credential',
      );
    }
    const format = await resolveOutput(command, opts, { allowed: ['pretty', 'json', 'yaml'] });
    // Reserve the destination before confirmation, OAuth, or the mutation. This guarantees
    // an existing/unwritable path cannot consume a one-time credential.
    if (requiresDestination && opts.secretOutput) {
      destinationHandle = await open(opts.secretOutput, 'wx', 0o600);
      destinationReserved = true;
    }
    if (prompt) {
      const action = prompt
        .replace(/\?$/, '')
        .replace(/^./, (character) => character.toLowerCase());
      await confirmOrAbort(prompt, Boolean(opts.force), {
        action,
        onAbort: cleanupReservedDestination,
      });
    }
    const client = await createAiGatewayClient();
    const result = await write(client, prepared);
    if (requiresDestination && opts.secretOutput && destinationHandle) {
      await destinationHandle.writeFile(`${JSON.stringify(result, null, 2)}\n`, 'utf8');
      secretPersisted = true;
      await destinationHandle.close();
      destinationHandle = undefined;
      renderGatewayDetail({ saved_to: opts.secretOutput }, format);
      return;
    }
    renderGatewayDetail(
      requiresDestination ? result : (settings.redactResponse?.(result) ?? result),
      format,
    );
  } catch (error) {
    await cleanupReservedDestination();
    failAiGateway(error);
  }
}
