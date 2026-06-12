import type {
  InitOptions,
  ModelSecurityClientOptions,
  RedTeamClientOptions,
} from '@cdot65/prisma-airs-sdk';
import type { Config } from './schema.js';

/** Build SDK `init()` options for the runtime scan API from CLI config. */
export function runtimeInitOptions(config: Config): InitOptions {
  return {
    apiKey: config.airsApiKey,
    apiToken: config.airsApiToken,
    apiEndpoint: config.airsApiEndpoint,
    numRetries: config.airsNumRetries,
  };
}

/** Build RedTeamClient options from CLI config. Creds are shared with mgmt*. */
export function redTeamClientOptions(config: Config): RedTeamClientOptions {
  return {
    clientId: config.mgmtClientId,
    clientSecret: config.mgmtClientSecret,
    tsgId: config.mgmtTsgId,
    dataEndpoint: config.redTeamDataEndpoint,
    mgmtEndpoint: config.redTeamMgmtEndpoint,
    tokenEndpoint: config.redTeamTokenEndpoint ?? config.mgmtTokenEndpoint,
  };
}

/** Build ModelSecurityClient options from CLI config. Creds are shared with mgmt*. */
export function modelSecurityClientOptions(config: Config): ModelSecurityClientOptions {
  return {
    clientId: config.mgmtClientId,
    clientSecret: config.mgmtClientSecret,
    tsgId: config.mgmtTsgId,
    dataEndpoint: config.modelSecDataEndpoint,
    mgmtEndpoint: config.modelSecMgmtEndpoint,
    tokenEndpoint: config.modelSecTokenEndpoint ?? config.mgmtTokenEndpoint,
  };
}
