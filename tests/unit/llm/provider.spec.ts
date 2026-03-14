import { describe, expect, it, vi } from 'vitest';
import type { LlmProvider } from '../../../src/config/schema.js';
import { createLlmProvider } from '../../../src/llm/provider.js';

const { ChatAnthropicMock, ChatVertexAIMock, ChatBedrockConverseMock, AnthropicVertexMock } =
  vi.hoisted(() => ({
    ChatAnthropicMock: vi.fn().mockImplementation((opts) => ({
      _type: 'ChatAnthropic',
      ...opts,
    })),
    ChatVertexAIMock: vi.fn().mockImplementation((opts) => ({
      _type: 'ChatVertexAI',
      ...opts,
    })),
    ChatBedrockConverseMock: vi.fn().mockImplementation((opts) => ({
      _type: 'ChatBedrockConverse',
      ...opts,
    })),
    AnthropicVertexMock: vi.fn().mockImplementation((opts) => ({
      _type: 'AnthropicVertex',
      ...opts,
    })),
  }));

vi.mock('@langchain/anthropic', () => ({
  ChatAnthropic: ChatAnthropicMock,
}));

vi.mock('@langchain/google-genai', () => ({
  ChatGoogleGenerativeAI: vi.fn().mockImplementation((opts) => ({
    _type: 'ChatGoogleGenerativeAI',
    ...opts,
  })),
}));

vi.mock('@langchain/google-vertexai', () => ({
  ChatVertexAI: ChatVertexAIMock,
}));

vi.mock('@langchain/aws', () => ({
  ChatBedrockConverse: ChatBedrockConverseMock,
}));

vi.mock('@anthropic-ai/vertex-sdk', () => ({
  AnthropicVertex: AnthropicVertexMock,
}));

describe('createLlmProvider', () => {
  it('creates Claude API provider', async () => {
    const model = await createLlmProvider({
      provider: 'claude-api',
      anthropicApiKey: 'test-key',
    });
    expect(model).toBeDefined();
  });

  it('creates Gemini API provider', async () => {
    const model = await createLlmProvider({
      provider: 'gemini-api',
      googleApiKey: 'test-key',
    });
    expect(model).toBeDefined();
  });

  it('creates Gemini Vertex provider with project and location', async () => {
    ChatVertexAIMock.mockClear();
    const model = await createLlmProvider({
      provider: 'gemini-vertex',
      googleCloudProject: 'my-project',
      googleCloudLocation: 'us-central1',
    });
    expect(model).toBeDefined();
    expect(ChatVertexAIMock).toHaveBeenCalledWith(
      expect.objectContaining({
        location: 'us-central1',
        authOptions: { projectId: 'my-project' },
      }),
    );
  });

  it('creates Claude Vertex provider with createClient', async () => {
    ChatAnthropicMock.mockClear();
    AnthropicVertexMock.mockClear();
    const model = await createLlmProvider({
      provider: 'claude-vertex',
      googleCloudProject: 'my-project',
      googleCloudLocation: 'us-east5',
    });
    expect(model).toBeDefined();
    expect(AnthropicVertexMock).toHaveBeenCalledWith({
      projectId: 'my-project',
      region: 'us-east5',
    });
    const callOpts = ChatAnthropicMock.mock.calls[0][0];
    expect(callOpts.createClient).toBeTypeOf('function');
    // Invoke createClient to cover the arrow function
    const client = callOpts.createClient();
    expect(client._type).toBe('AnthropicVertex');
  });

  it('creates Claude Bedrock provider', async () => {
    ChatBedrockConverseMock.mockClear();
    const model = await createLlmProvider({
      provider: 'claude-bedrock',
      awsRegion: 'us-east-1',
    });
    expect(model).toBeDefined();
    // No credentials when not provided
    expect(ChatBedrockConverseMock).toHaveBeenCalledWith(
      expect.not.objectContaining({ credentials: expect.anything() }),
    );
  });

  it('creates Claude Bedrock provider with explicit credentials', async () => {
    ChatBedrockConverseMock.mockClear();
    const model = await createLlmProvider({
      provider: 'claude-bedrock',
      awsRegion: 'us-west-2',
      awsAccessKeyId: 'AKID',
      awsSecretAccessKey: 'SECRET',
    });
    expect(model).toBeDefined();
    expect(ChatBedrockConverseMock).toHaveBeenCalledWith(
      expect.objectContaining({
        credentials: {
          accessKeyId: 'AKID',
          secretAccessKey: 'SECRET',
        },
        region: 'us-west-2',
      }),
    );
  });

  it('creates Gemini Bedrock provider', async () => {
    const model = await createLlmProvider({
      provider: 'gemini-bedrock',
      awsRegion: 'us-east-1',
    });
    expect(model).toBeDefined();
  });

  it('uses default region for claude-vertex when not provided', async () => {
    AnthropicVertexMock.mockClear();
    await createLlmProvider({ provider: 'claude-vertex', googleCloudProject: 'proj' });
    expect(AnthropicVertexMock).toHaveBeenCalledWith({ projectId: 'proj', region: 'global' });
  });

  it('uses default region for claude-bedrock when not provided', async () => {
    ChatBedrockConverseMock.mockClear();
    await createLlmProvider({ provider: 'claude-bedrock' });
    expect(ChatBedrockConverseMock).toHaveBeenCalledWith(
      expect.objectContaining({ region: 'us-east-1' }),
    );
  });

  it('uses default location for gemini-vertex when not provided', async () => {
    ChatVertexAIMock.mockClear();
    await createLlmProvider({ provider: 'gemini-vertex', googleCloudProject: 'proj' });
    expect(ChatVertexAIMock).toHaveBeenCalledWith(
      expect.objectContaining({ location: 'us-central1' }),
    );
  });

  it('uses default region for gemini-bedrock when not provided', async () => {
    ChatBedrockConverseMock.mockClear();
    await createLlmProvider({ provider: 'gemini-bedrock' });
    expect(ChatBedrockConverseMock).toHaveBeenCalledWith(
      expect.objectContaining({ region: 'us-east-1' }),
    );
  });

  it('throws for unknown provider', async () => {
    await expect(createLlmProvider({ provider: 'unknown' as LlmProvider })).rejects.toThrow();
  });
});
