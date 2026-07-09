import { z } from 'zod';

export const ConfigSchema = z.object({
  // AIRS Scanner (runtime scan API)
  airsApiKey: z.string().optional(),
  airsApiToken: z.string().optional(),
  airsApiEndpoint: z.string().optional(),
  airsNumRetries: z.coerce.number().int().min(0).max(5).optional(),

  // AIRS Management (SDK v2 — OAuth2 client credentials)
  mgmtClientId: z.string().optional(),
  mgmtClientSecret: z.string().optional(),
  mgmtTsgId: z.string().optional(),
  mgmtEndpoint: z.string().optional(),
  mgmtTokenEndpoint: z.string().optional(),
  dlpEndpoint: z.string().optional(),

  // Red Team (endpoints only; creds shared with mgmt*)
  redTeamDataEndpoint: z.string().optional(),
  redTeamMgmtEndpoint: z.string().optional(),
  redTeamTokenEndpoint: z.string().optional(),
  redTeamNetworkBrokerEndpoint: z.string().optional(),

  // Model Security (endpoints only; creds shared with mgmt*)
  modelSecDataEndpoint: z.string().optional(),
  modelSecMgmtEndpoint: z.string().optional(),
  modelSecTokenEndpoint: z.string().optional(),

  // Tuning
  scanConcurrency: z.coerce.number().int().min(1).max(20).default(5),

  // Persistence
  dataDir: z.string().default('~/.prisma-airs/runs'),
});

export type Config = z.infer<typeof ConfigSchema>;
