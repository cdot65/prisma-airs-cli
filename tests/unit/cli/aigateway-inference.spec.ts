import { Command } from 'commander';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
  inferenceBody,
  registerAiGatewayInference,
} from '../../../src/cli/commands/aigateway/inference.js';

const mock = vi.hoisted(() => ({
  chat: vi.fn(),
  responses: vi.fn(),
  embeddings: vi.fn(),
  constructor: vi.fn(),
  file: vi.fn(),
}));
vi.mock('@cdot65/prisma-airs-sdk', async (original) => ({
  ...(await original<typeof import('@cdot65/prisma-airs-sdk')>()),
  AIGatewayInferenceClient: class {
    constructor(options: unknown) {
      mock.constructor(options);
    }
    createChatCompletion = mock.chat;
    createResponse = mock.responses;
    createEmbedding = mock.embeddings;
  },
}));
vi.mock('../../../src/config/loader.js', () => ({
  loadConfig: vi.fn(async (overrides = {}) => ({
    aiGwInferenceEndpoint: 'https://gateway.test/v1',
    aiGwInferenceApiKey: 'test-only-runtime-key',
    aiGwInferenceModel: '@provider/chat',
    aiGwEmbeddingModel: '@provider/embed',
    ...overrides,
  })),
}));
vi.mock('node:fs/promises', () => ({ readFile: mock.file }));

let output: string[];
let exit: ReturnType<typeof vi.spyOn>;
function program() {
  const command = new Command().exitOverride();
  registerAiGatewayInference(command);
  return command;
}
async function run(args: string[]) {
  await program().parseAsync(['inference', ...args], { from: 'user' });
}
beforeEach(() => {
  vi.clearAllMocks();
  output = [];
  vi.spyOn(process.stdout, 'write').mockImplementation((chunk, encoding, callback) => {
    output.push(String(chunk));
    const done = typeof encoding === 'function' ? encoding : callback;
    done?.();
    return true;
  });
  vi.spyOn(console, 'error').mockImplementation(() => {});
  exit = vi.spyOn(process, 'exit').mockImplementation((code) => {
    throw new Error(`EXIT:${code}`);
  });
  mock.chat.mockResolvedValue({ choices: [{ message: { content: 'READY' } }] });
  mock.responses.mockResolvedValue({
    status: 'completed',
    output: [{ content: [{ type: 'output_text', text: 'READY' }] }],
  });
  mock.embeddings.mockResolvedValue({ data: [{ embedding: [1, 2] }] });
});
afterEach(() => vi.restoreAllMocks());

describe('gateway inference CLI', () => {
  it('prints the non-streaming SDK response as JSON without exposing credentials', async () => {
    await run([
      'chat',
      'hello',
      '--model',
      '@openai/gpt-5.6-terra',
      '--endpoint',
      'https://gateway.test/v1',
      '--output',
      'json',
    ]);
    expect(JSON.parse(output.join(''))).toEqual({ choices: [{ message: { content: 'READY' } }] });
    expect(mock.chat.mock.calls[0][0]).toMatchObject({ model: '@openai/gpt-5.6-terra' });
    expect(mock.constructor.mock.calls[0][0]).toMatchObject({
      apiKey: 'test-only-runtime-key',
      numRetries: 0,
    });
    expect(output.join('')).not.toContain('test-only-runtime-key');
  });
  it('uses the configured embedding model and numeric dimensions', async () => {
    await run([
      'embeddings',
      'hello',
      '--dimensions',
      '32',
      '--encoding',
      'base64',
      '--output',
      'json',
    ]);
    expect(mock.embeddings.mock.calls[0][0]).toEqual({
      model: '@provider/embed',
      input: 'hello',
      dimensions: 32,
      encoding_format: 'base64',
    });
  });
  it('defaults ordinary Responses calls to non-persistent storage', async () => {
    await run(['responses', 'hello', '--max-tokens', '128']);
    expect(mock.responses.mock.calls[0][0]).toMatchObject({ store: false, max_output_tokens: 128 });
    expect(output.join('')).toBe('READY\n');
  });
  it.each([
    ['chat'],
    ['chat', 'hello', '--max-tokens', '-1'],
    ['chat', 'hello', '--stream', '--output', 'yaml'],
    ['embeddings', 'hello', '--encoding', 'invalid'],
    ['chat', 'hello', '--timeout', 'NaN'],
  ])('fails usage before sending a request: %j', async (...args) => {
    await expect(run(args)).rejects.toThrow('EXIT:2');
    expect(mock.chat).not.toHaveBeenCalled();
    expect(mock.embeddings).not.toHaveBeenCalled();
  });
  it('emits JSONL chunks and releases signal handlers after a stream', async () => {
    const cancel = vi.fn(async () => {});
    const before = process.listenerCount('SIGINT');
    const events = [
      { choices: [{ delta: { content: 'one' } }] },
      { choices: [{ delta: { content: 'two' } }] },
    ];
    mock.chat.mockResolvedValue(
      Object.assign(
        (async function* () {
          yield* events;
        })(),
        { cancel },
      ),
    );
    await run(['chat', 'hello', '--stream', '--output', 'json']);
    expect(
      output
        .join('')
        .trim()
        .split('\n')
        .map((line) => JSON.parse(line)),
    ).toEqual(events);
    expect(cancel).toHaveBeenCalledOnce();
    expect(process.listenerCount('SIGINT')).toBe(before);
  });
  it('cleans up a failed stream before invoking the process-exit helper', async () => {
    const cancel = vi.fn(async () => {});
    mock.chat.mockResolvedValue(
      Object.assign(
        (async function* () {
          yield { choices: [] };
          throw new Error('truncated stream');
        })(),
        { cancel },
      ),
    );
    await expect(run(['chat', 'hello', '--stream'])).rejects.toThrow('EXIT:1');
    expect(cancel).toHaveBeenCalledOnce();
    expect(cancel.mock.invocationCallOrder[0]).toBeLessThan(exit.mock.invocationCallOrder[0]);
  });
  it('treats an incomplete Responses lifecycle as an operational failure', async () => {
    const cancel = vi.fn(async () => {});
    mock.responses.mockResolvedValue(
      Object.assign(
        (async function* () {
          yield { type: 'response.incomplete' };
        })(),
        { cancel },
      ),
    );
    await expect(run(['responses', 'hello', '--stream', '--output', 'json'])).rejects.toThrow(
      'EXIT:1',
    );
    expect(cancel).toHaveBeenCalledOnce();
  });
  it('honors file input and explicit overrides without mutating stored settings', async () => {
    mock.file.mockResolvedValue(
      JSON.stringify({
        model: '@file/model',
        messages: [{ role: 'user', content: 'private' }],
        temperature: 0,
      }),
    );
    const body = await inferenceBody(
      'chat',
      undefined,
      { file: 'request.json', model: '@flag/model', maxTokens: '64' },
      '@config/model',
    );
    expect(body).toMatchObject({ model: '@flag/model', temperature: 0, max_completion_tokens: 64 });
    await expect(inferenceBody('chat', 'hello', { file: 'request.json' })).rejects.toThrow(
      'not both',
    );
  });
  it('rejects unreadable, malformed and nonobject files without echoing their contents', async () => {
    for (const value of ['{secret-invalid', '[]', 'null', '42']) {
      mock.file.mockResolvedValue(value);
      await expect(inferenceBody('chat', undefined, { file: 'request.json' })).rejects.not.toThrow(
        'secret-invalid',
      );
    }
    mock.file.mockRejectedValue(new Error('unreadable'));
    await expect(inferenceBody('chat', undefined, { file: 'request.json' })).rejects.toThrow(
      'readable JSON',
    );
  });
});
