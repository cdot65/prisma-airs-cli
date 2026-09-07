import type {
  AIGatewayClientOptions,
  InitOptions,
  ManagementClientOptions,
  ModelSecurityClientOptions,
  RedTeamClientOptions,
} from '@cdot65/prisma-airs-sdk';
import type { Config } from './schema.js';

/** SCM dashboard host is scoped separately; other management resources keep their endpoint. */
export function managementClientOptions(config: Config): ManagementClientOptions {
  return {
    clientId: config.mgmtClientId,
    clientSecret: config.mgmtClientSecret,
    tsgId: config.mgmtTsgId,
    apiEndpoint: config.mgmtEndpoint,
    dashboardEndpoint:
      config.mgmtDashboardEndpoint ?? 'https://api.apps.paloaltonetworks.com/aisec',
    tokenEndpoint: config.mgmtTokenEndpoint,
    dlpEndpoint: config.dlpEndpoint,
  };
}

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
    networkBrokerEndpoint: config.redTeamNetworkBrokerEndpoint,
  };
}

/** Build AIGatewayClient options from CLI config. Creds are shared with mgmt*. */
export function aiGatewayClientOptions(config: Config): AIGatewayClientOptions {
  return {
    clientId: config.mgmtClientId,
    clientSecret: config.mgmtClientSecret,
    tsgId: config.mgmtTsgId,
    dataEndpoint: config.aiGwDataEndpoint,
    adminEndpoint: config.aiGwAdminEndpoint,
    tokenEndpoint: config.aiGwTokenEndpoint ?? config.mgmtTokenEndpoint,
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
