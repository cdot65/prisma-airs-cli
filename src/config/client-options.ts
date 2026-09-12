import {
  AGENT_GUARD_DATA_ENDPOINT,
  AGENT_GUARD_MGMT_ENDPOINT,
  type AgentGuardClientOptions,
  type AIGatewayClientOptions,
  DEFAULT_AI_GW_ADMIN_ENDPOINT,
  DEFAULT_AI_GW_DATA_ENDPOINT,
  DEFAULT_DLP_ENDPOINT,
  DEFAULT_ENDPOINT,
  DEFAULT_IAM_ENDPOINT,
  DEFAULT_MGMT_ENDPOINT,
  DEFAULT_MODEL_SEC_DATA_ENDPOINT,
  DEFAULT_MODEL_SEC_MGMT_ENDPOINT,
  DEFAULT_RED_TEAM_DATA_ENDPOINT,
  DEFAULT_RED_TEAM_MGMT_ENDPOINT,
  DEFAULT_RED_TEAM_NETWORK_BROKER_ENDPOINT,
  DEFAULT_TOKEN_ENDPOINT,
  type InitOptions,
  type ManagementClientOptions,
  type ModelSecurityClientOptions,
  type RedTeamClientOptions,
} from '@cdot65/prisma-airs-sdk';
import { assertManagementCredentials, assertScannerCredentials } from './credentials.js';
import type { Config } from './schema.js';

/** SCM dashboard host; the SDK alone would reuse the management endpoint. */
export const DEFAULT_MGMT_DASHBOARD_ENDPOINT = 'https://api.apps.paloaltonetworks.com/aisec';

// Every builder passes a concrete value for each option the SDK could otherwise read
// from the environment (credentials, token endpoint, base URLs). The tenant file is
// therefore the only input, and every management-plane product shares one OAuth
// client ID, secret, TSG, and token endpoint. Base URLs default to SDK constants.

function oauth(config: Config) {
  assertManagementCredentials(config);
  return {
    clientId: config.mgmtClientId,
    clientSecret: config.mgmtClientSecret,
    tsgId: config.mgmtTsgId,
    tokenEndpoint: config.mgmtTokenEndpoint ?? DEFAULT_TOKEN_ENDPOINT,
  };
}

export function managementClientOptions(config: Config): ManagementClientOptions {
  return {
    ...oauth(config),
    apiEndpoint: config.mgmtEndpoint ?? DEFAULT_MGMT_ENDPOINT,
    dashboardEndpoint: config.mgmtDashboardEndpoint ?? DEFAULT_MGMT_DASHBOARD_ENDPOINT,
    dlpEndpoint: config.dlpEndpoint ?? DEFAULT_DLP_ENDPOINT,
  };
}

export function agentGuardClientOptions(config: Config): AgentGuardClientOptions {
  return {
    ...oauth(config),
    dataEndpoint: config.agentGuardDataEndpoint ?? AGENT_GUARD_DATA_ENDPOINT,
    mgmtEndpoint: config.agentGuardMgmtEndpoint ?? AGENT_GUARD_MGMT_ENDPOINT,
  };
}

export function redTeamClientOptions(config: Config): RedTeamClientOptions {
  return {
    ...oauth(config),
    dataEndpoint: config.redTeamDataEndpoint ?? DEFAULT_RED_TEAM_DATA_ENDPOINT,
    mgmtEndpoint: config.redTeamMgmtEndpoint ?? DEFAULT_RED_TEAM_MGMT_ENDPOINT,
    networkBrokerEndpoint:
      config.redTeamNetworkBrokerEndpoint ?? DEFAULT_RED_TEAM_NETWORK_BROKER_ENDPOINT,
  };
}

export function modelSecurityClientOptions(config: Config): ModelSecurityClientOptions {
  return {
    ...oauth(config),
    dataEndpoint: config.modelSecDataEndpoint ?? DEFAULT_MODEL_SEC_DATA_ENDPOINT,
    mgmtEndpoint: config.modelSecMgmtEndpoint ?? DEFAULT_MODEL_SEC_MGMT_ENDPOINT,
  };
}

export function aiGatewayClientOptions(config: Config): AIGatewayClientOptions {
  return {
    ...oauth(config),
    dataEndpoint: config.aiGwDataEndpoint ?? DEFAULT_AI_GW_DATA_ENDPOINT,
    adminEndpoint: config.aiGwAdminEndpoint ?? DEFAULT_AI_GW_ADMIN_ENDPOINT,
    iamEndpoint: config.iamEndpoint ?? DEFAULT_IAM_ENDPOINT,
  };
}

/** SDK `init()` options for the runtime scan API. */
export function runtimeInitOptions(config: Config): InitOptions {
  assertScannerCredentials(config);
  return {
    apiKey: config.airsApiKey,
    apiToken: config.airsApiToken,
    apiEndpoint: config.airsApiEndpoint ?? DEFAULT_ENDPOINT,
    numRetries: config.airsNumRetries,
  };
}
