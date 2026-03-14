import { z } from 'zod';

export const LlmProviderSchema = z.enum([
  'claude-api',
  'claude-vertex',
  'claude-bedrock',
  'gemini-api',
  'gemini-vertex',
  'gemini-bedrock',
]);

export type LlmProvider = z.infer<typeof LlmProviderSchema>;

export const ConfigSchema = z.object({
  llmProvider: LlmProviderSchema.default('claude-api'),
  llmModel: z.string().optional(),

  // Claude API
  anthropicApiKey: z.string().optional(),

  // Google AI
  googleApiKey: z.string().optional(),

  // Google Vertex
  googleCloudProject: z.string().optional(),
  googleCloudLocation: z.string().default('us-central1'),

  // AWS Bedrock
  awsRegion: z.string().default('us-east-1'),
  awsAccessKeyId: z.string().optional(),
  awsSecretAccessKey: z.string().optional(),

  // AIRS Scanner
  airsApiKey: z.string().optional(),

  // AIRS Management (SDK v2 — OAuth2 client credentials)
  mgmtClientId: z.string().optional(),
  mgmtClientSecret: z.string().optional(),
  mgmtTsgId: z.string().optional(),
  mgmtEndpoint: z.string().optional(),
  mgmtTokenEndpoint: z.string().optional(),

  // Tuning
  scanConcurrency: z.coerce.number().int().min(1).max(20).default(5),
  accumulateTests: z.coerce.boolean().default(false),
  maxAccumulatedTests: z.coerce.number().int().min(1).optional(),

  // Persistence
  dataDir: z.string().default('~/.prisma-airs/runs'),

  // Memory
  memoryEnabled: z.coerce.boolean().default(true),
  memoryDir: z.string().default('~/.prisma-airs/memory'),
  maxMemoryChars: z.coerce.number().int().min(500).max(10000).default(3000),
});

export type Config = z.infer<typeof ConfigSchema>;
