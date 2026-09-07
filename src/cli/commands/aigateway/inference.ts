import { readFile } from 'node:fs/promises';
import {
  AIGatewayInferenceClient,
  ErrorType,
  GatewayInferenceInputCreateChatCompletionRequestSchema,
  GatewayInferenceInputCreateEmbeddingRequestSchema,
  GatewayInferenceInputCreateResponseSchema,
} from '@cdot65/prisma-airs-sdk';
import type { Command } from 'commander';
import { dump } from 'js-yaml';
import { loadConfig } from '../../../config/loader.js';
import { CliUsageError, fail, resolveOutput, usageError } from '../../renderer/common.js';

type Kind = 'chat' | 'responses' | 'embeddings';
interface Options {
  model?: string;
  endpoint?: string;
  file?: string;
  stream?: boolean;
  maxTokens?: string;
  dimensions?: string;
  encoding?: string;
  timeout?: string;
  output?: string;
}

function integer(raw: string, flag: string): number {
  const value = Number(raw);
  if (!Number.isSafeInteger(value) || value < 1)
    throw new CliUsageError(`${flag} must be a positive integer`);
  return value;
}

/** Build only explicitly requested overrides; the SDK performs the complete body validation. */
export async function inferenceBody(
  kind: Kind,
  prompt: string | undefined,
  opts: Options,
  defaultModel?: string,
): Promise<Record<string, unknown>> {
  if (prompt !== undefined && opts.file)
    throw new CliUsageError('Pass a prompt or --file, not both');
  if (prompt === undefined && !opts.file) throw new CliUsageError('A prompt or --file is required');
  let body: Record<string, unknown>;
  if (opts.file) {
    let value: unknown;
    try {
      value = JSON.parse(await readFile(opts.file, 'utf8'));
    } catch {
      throw new CliUsageError('--file must name a readable JSON request file');
    }
    if (!value || typeof value !== 'object' || Array.isArray(value))
      throw new CliUsageError('--file must contain a JSON object');
    body = value as Record<string, unknown>;
  } else {
    body = kind === 'chat' ? { messages: [{ role: 'user', content: prompt }] } : { input: prompt };
  }
  body = { ...body };
  const model = opts.model ?? body.model ?? defaultModel;
  if (typeof model !== 'string' || !model.trim())
    throw new CliUsageError(
      'Set --model, the request model, or the matching inference model config',
    );
  body.model = model;
  if (opts.stream !== undefined) body.stream = opts.stream;
  if (opts.maxTokens !== undefined)
    body[kind === 'responses' ? 'max_output_tokens' : 'max_completion_tokens'] = integer(
      opts.maxTokens,
      '--max-tokens',
    );
  if (opts.dimensions !== undefined) body.dimensions = integer(opts.dimensions, '--dimensions');
  if (opts.encoding !== undefined) body.encoding_format = opts.encoding;
  // Responses storage is opt-in from a file; ordinary CLI prompts need not persist provider state.
  if (kind === 'responses' && body.store === undefined) body.store = false;
  return body;
}

function responseText(value: unknown): string {
  if (!value || typeof value !== 'object') return '';
  const record = value as Record<string, unknown>;
  if (record.type === 'response.output_text.delta' && typeof record.delta === 'string')
    return record.delta;
  if (Array.isArray(record.choices))
    return record.choices
      .map((choice) => choice.message?.content ?? choice.delta?.content ?? '')
      .join('');
  if (Array.isArray(record.output))
    return record.output
      .flatMap((item) => (Array.isArray(item.content) ? item.content : []))
      .filter((part) => part.type === 'output_text')
      .map((part) => part.text)
      .join('');
  return '';
}

/** Register key-authenticated inference separately from OAuth management commands. */
export function registerAiGatewayInference(parent: Command): void {
  const group = parent
    .command('inference')
    .description('Call the gateway runtime (separate API key; no SCM OAuth required)');
  for (const kind of ['chat', 'responses', 'embeddings'] as const) {
    const command = group
      .command(`${kind} [prompt]`)
      .description(
        `${kind === 'embeddings' ? 'Create embeddings' : 'Generate a model response'} through your runtime gateway`,
      )
      .option('--model <id>', 'Provider-prefixed model ID; overrides request file and config')
      .option('--endpoint <url>', 'Runtime base URL including /v1; overrides config')
      .option('--file <path>', 'JSON request file (mutually exclusive with prompt)')
      .option('--timeout <ms>', 'Total deadline including streaming, in milliseconds', '60000')
      .option(
        '--output <format>',
        'pretty, json or yaml; streaming json emits one chunk per line (JSONL)',
      );
    if (kind === 'embeddings')
      command
        .option('--dimensions <n>', 'Embedding dimensions')
        .option('--encoding <format>', 'float or base64');
    else
      command
        .option('--stream', 'Stream text (pretty) or validated events (json/JSONL)')
        .option('--max-tokens <n>', 'Maximum completion/output tokens');
    command.action(async (prompt: string | undefined, opts: Options) => {
      const controller = new AbortController();
      const interrupt = () => controller.abort(new Error('Inference interrupted'));
      const outputError = (error: Error) => controller.abort(error);
      let stream: { cancel(reason?: unknown): Promise<void> } | undefined;
      let failed = false;
      let failure: unknown;
      try {
        const format = await resolveOutput(command, opts, { allowed: ['pretty', 'json', 'yaml'] });
        const config = await loadConfig({ aiGwInferenceEndpoint: opts.endpoint });
        const body = await inferenceBody(
          kind,
          prompt,
          opts,
          kind === 'embeddings' ? config.aiGwEmbeddingModel : config.aiGwInferenceModel,
        );
        if (body.stream === true && format === 'yaml')
          throw new CliUsageError('Streaming supports --output pretty or json (JSONL), not yaml');
        const schema =
          kind === 'chat'
            ? GatewayInferenceInputCreateChatCompletionRequestSchema
            : kind === 'responses'
              ? GatewayInferenceInputCreateResponseSchema
              : GatewayInferenceInputCreateEmbeddingRequestSchema;
        const parsed = schema.safeParse(body);
        if (!parsed.success)
          throw new CliUsageError(
            `Invalid inference request: ${parsed.error.issues.map((issue) => `${issue.path.join('.') || '<root>'}: ${issue.code}`).join('; ')}`,
          );
        const client = new AIGatewayInferenceClient({
          endpoint: config.aiGwInferenceEndpoint,
          apiKey: config.aiGwInferenceApiKey,
          timeoutMs: integer(opts.timeout ?? '60000', '--timeout'),
          numRetries: 0,
        });
        process.once('SIGINT', interrupt);
        process.once('SIGTERM', interrupt);
        process.stdout.on('error', outputError);
        const write = (text: string) =>
          new Promise<void>((resolve, reject) => {
            process.stdout.write(text, (error) => (error ? reject(error) : resolve()));
          });
        const requestOptions = { signal: controller.signal };
        const result =
          kind === 'chat'
            ? await client.createChatCompletion(
                GatewayInferenceInputCreateChatCompletionRequestSchema.parse(body),
                requestOptions,
              )
            : kind === 'responses'
              ? await client.createResponse(
                  GatewayInferenceInputCreateResponseSchema.parse(body),
                  requestOptions,
                )
              : await client.createEmbedding(
                  GatewayInferenceInputCreateEmbeddingRequestSchema.parse(body),
                  requestOptions,
                );
        if (Symbol.asyncIterator in result) {
          stream = result;
          for await (const event of result) {
            if (format === 'json') await write(`${JSON.stringify(event)}\n`);
            else {
              const text = responseText(event);
              if (text) await write(text);
            }
            if (
              'type' in event &&
              (event.type === 'response.failed' || event.type === 'response.incomplete')
            )
              throw new Error(`Gateway generation ended with ${event.type}`);
          }
          if (format === 'pretty') await write('\n');
        } else {
          const text =
            format === 'yaml'
              ? dump(result, { noRefs: true }).trimEnd()
              : format === 'pretty' && kind !== 'embeddings'
                ? responseText(result)
                : JSON.stringify(result, null, 2);
          await write(`${text}\n`);
          if ('status' in result && (result.status === 'failed' || result.status === 'incomplete'))
            throw new Error(`Gateway generation ended with ${result.status}`);
        }
      } catch (error) {
        failed = true;
        failure = error;
      } finally {
        await stream?.cancel();
        controller.abort();
        process.removeListener('SIGINT', interrupt);
        process.removeListener('SIGTERM', interrupt);
        process.stdout.removeListener('error', outputError);
      }
      // Cleanup precedes helpers that exit the process. head/other pipe consumers may close
      // intentionally; every other operational error remains a nonzero exit.
      if (failed && (failure as NodeJS.ErrnoException)?.code !== 'EPIPE') {
        if ((failure as { errorType?: string })?.errorType === ErrorType.USER_REQUEST_PAYLOAD_ERROR)
          usageError(failure instanceof Error ? failure.message : 'Invalid inference options');
        fail(failure);
      }
    });
  }
}
