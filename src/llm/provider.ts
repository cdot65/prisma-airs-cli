import { ChatAnthropic } from '@langchain/anthropic';
import { ChatBedrockConverse } from '@langchain/aws';
import type { BaseChatModel } from '@langchain/core/language_models/chat_models';
import { ChatGoogleGenerativeAI } from '@langchain/google-genai';
import { ChatVertexAI } from '@langchain/google-vertexai';
import type { LlmProvider } from '../config/schema.js';

/** Configuration for creating an LLM provider instance. */
export interface LlmProviderConfig {
  provider: LlmProvider;
  model?: string;
  anthropicApiKey?: string;
  googleApiKey?: string;
  googleCloudProject?: string;
  googleCloudLocation?: string;
  awsRegion?: string;
  awsAccessKeyId?: string;
  awsSecretAccessKey?: string;
}

const DEFAULT_MODELS: Record<LlmProvider, string> = {
  'claude-api': 'claude-opus-4-6',
  'claude-vertex': 'claude-opus-4-6',
  'claude-bedrock': 'anthropic.claude-opus-4-6-v1',
  'gemini-api': 'gemini-2.5-pro',
  'gemini-vertex': 'gemini-2.5-pro',
  'gemini-bedrock': 'gemini-2.5-pro',
};

/**
 * Factory that creates a LangChain chat model for the configured provider.
 * @param config - Provider type, model name, and auth credentials.
 * @returns A configured LangChain BaseChatModel instance.
 */
export async function createLlmProvider(config: LlmProviderConfig): Promise<BaseChatModel> {
  const modelName = config.model ?? DEFAULT_MODELS[config.provider];

  switch (config.provider) {
    case 'claude-api':
      return new ChatAnthropic({
        model: modelName,
        anthropicApiKey: config.anthropicApiKey,
        temperature: 0,
      });

    case 'claude-vertex': {
      const { AnthropicVertex } = await import('@anthropic-ai/vertex-sdk');
      const client = new AnthropicVertex({
        projectId: config.googleCloudProject,
        region: config.googleCloudLocation ?? 'global',
      });
      return new ChatAnthropic({
        model: modelName,
        temperature: 0,
        createClient: () => client,
      });
    }

    case 'claude-bedrock': {
      const opts: Record<string, unknown> = {
        model: modelName,
        region: config.awsRegion ?? 'us-east-1',
        temperature: 0,
      };
      if (config.awsAccessKeyId && config.awsSecretAccessKey) {
        opts.credentials = {
          accessKeyId: config.awsAccessKeyId,
          secretAccessKey: config.awsSecretAccessKey,
        };
      }
      return new ChatBedrockConverse(opts);
    }

    case 'gemini-api':
      return new ChatGoogleGenerativeAI({
        model: modelName,
        apiKey: config.googleApiKey,
        temperature: 0,
      });

    case 'gemini-vertex':
      return new ChatVertexAI({
        model: modelName,
        location: config.googleCloudLocation ?? 'us-central1',
        temperature: 0,
        authOptions: { projectId: config.googleCloudProject },
      });

    case 'gemini-bedrock':
      return new ChatBedrockConverse({
        model: modelName,
        region: config.awsRegion ?? 'us-east-1',
        temperature: 0,
      });

    default:
      throw new Error(`Unknown LLM provider: ${config.provider}`);
  }
}
