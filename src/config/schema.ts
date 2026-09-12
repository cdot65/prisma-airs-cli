import { z } from 'zod';

/**
 * Everything a tenant config file may contain. There is exactly one credential set
 * and one OAuth token endpoint for every management-plane product (Management, DLP,
 * Red Team, Model Security, AgentGuard, AI Gateway admin/data planes, SCM IAM): the
 * `mgmt*` values below. Product base URLs default to the SDK constants; the optional
 * overrides exist for test servers and private routing, never for authentication.
 */
export const ConfigSchema = z.object({
  // AIRS Scanner (runtime scan API) — separate key-based auth
  airsApiKey: z.string().optional(),
  airsApiToken: z.string().optional(),
  airsApiEndpoint: z.string().optional(),
  airsNumRetries: z.coerce.number().int().min(0).max(5).optional(),

  // Shared SCM OAuth2 client credentials
  mgmtClientId: z.string().optional(),
  mgmtClientSecret: z.string().optional(),
  mgmtTsgId: z.string().optional(),
  mgmtEndpoint: z.string().optional(),
  mgmtDashboardEndpoint: z.string().url().optional(),
  mgmtTokenEndpoint: z.string().optional(),

  // Product base-URL overrides (authentication always comes from mgmt*)
  dlpEndpoint: z.string().optional(),
  redTeamDataEndpoint: z.string().optional(),
  redTeamMgmtEndpoint: z.string().optional(),
  redTeamNetworkBrokerEndpoint: z.string().optional(),
  modelSecDataEndpoint: z.string().optional(),
  modelSecMgmtEndpoint: z.string().optional(),
  agentGuardDataEndpoint: z.string().url().optional(),
  agentGuardMgmtEndpoint: z.string().url().optional(),
  aiGwDataEndpoint: z.string().optional(),
  aiGwAdminEndpoint: z.string().optional(),
  iamEndpoint: z.string().optional(),

  // AI Gateway runtime inference authenticates with a workspace key, never SCM OAuth.
  aiGwInferenceEndpoint: z.string().optional(),
  aiGwInferenceApiKey: z.string().optional(),
  aiGwInferenceModel: z.string().optional(),
  aiGwEmbeddingModel: z.string().optional(),

  // Tuning
  scanConcurrency: z.coerce.number().int().min(1).max(20).default(5),
  defaultOutput: z.enum(['pretty', 'table', 'markdown', 'csv', 'json', 'yaml']).optional(),

  // Persistence
  dataDir: z.string().default('~/.prisma-airs/runs'),
});

export type Config = z.infer<typeof ConfigSchema>;

export const CONFIG_KEYS = Object.keys(ConfigSchema.shape) as Array<keyof Config>;

/**
 * Per-product OAuth token endpoints from earlier releases. Every product now
 * authenticates through `mgmtTokenEndpoint`; these keys are ignored when present.
 */
export const RETIRED_CONFIG_KEYS = [
  'redTeamTokenEndpoint',
  'modelSecTokenEndpoint',
  'agentGuardTokenEndpoint',
  'aiGwTokenEndpoint',
] as const;
