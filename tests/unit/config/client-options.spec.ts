import {
  AGENT_GUARD_DATA_ENDPOINT,
  AGENT_GUARD_MGMT_ENDPOINT,
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
} from '@cdot65/prisma-airs-sdk';
import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  agentGuardClientOptions,
  aiGatewayClientOptions,
  DEFAULT_MGMT_DASHBOARD_ENDPOINT,
  managementClientOptions,
  modelSecurityClientOptions,
  redTeamClientOptions,
  runtimeInitOptions,
} from '../../../src/config/client-options.js';
import { ConfigSchema } from '../../../src/config/schema.js';

const CREDS = { mgmtClientId: 'client', mgmtClientSecret: 'secret', mgmtTsgId: 'tenant' };
const OAUTH = { clientId: 'client', clientSecret: 'secret', tsgId: 'tenant' };

afterEach(() => vi.unstubAllEnvs());

describe('one credential set for every management-plane product', () => {
  it.each([
    ['managementClientOptions', managementClientOptions],
    ['redTeamClientOptions', redTeamClientOptions],
    ['modelSecurityClientOptions', modelSecurityClientOptions],
    ['aiGatewayClientOptions', aiGatewayClientOptions],
    ['agentGuardClientOptions', agentGuardClientOptions],
  ])('%s uses the shared OAuth credentials and token endpoint', (_name, build) => {
    expect(build(ConfigSchema.parse(CREDS))).toMatchObject({
      ...OAUTH,
      tokenEndpoint: DEFAULT_TOKEN_ENDPOINT,
    });
    expect(
      build(ConfigSchema.parse({ ...CREDS, mgmtTokenEndpoint: 'https://auth.example/token' }))
        .tokenEndpoint,
    ).toBe('https://auth.example/token');
  });

  it.each([
    ['managementClientOptions', managementClientOptions],
    ['redTeamClientOptions', redTeamClientOptions],
    ['modelSecurityClientOptions', modelSecurityClientOptions],
    ['aiGatewayClientOptions', aiGatewayClientOptions],
    ['agentGuardClientOptions', agentGuardClientOptions],
  ])('%s refuses to build without the shared credential set', (_name, build) => {
    expect(() => build(ConfigSchema.parse({ mgmtClientId: 'client' }))).toThrow(
      /Management credentials are not configured \(missing mgmtClientSecret, mgmtTsgId\)/,
    );
  });

  it('never leaves any option undefined for the SDK to resolve from the environment', () => {
    vi.stubEnv('PANW_RED_TEAM_CLIENT_ID', 'legacy');
    vi.stubEnv('PANW_RED_TEAM_DATA_ENDPOINT', 'https://legacy.example');
    vi.stubEnv('PANW_MGMT_TOKEN_ENDPOINT', 'https://legacy.example');
    for (const build of [
      managementClientOptions,
      redTeamClientOptions,
      modelSecurityClientOptions,
      aiGatewayClientOptions,
      agentGuardClientOptions,
    ]) {
      const options = build(ConfigSchema.parse(CREDS)) as Record<string, unknown>;
      for (const [key, value] of Object.entries(options)) expect(value, key).toBeDefined();
      expect(JSON.stringify(options)).not.toContain('legacy');
    }
    expect(() => redTeamClientOptions(ConfigSchema.parse({}))).toThrow('not configured');
  });
});

describe('endpoints default to SDK constants and honor file-only base-URL overrides', () => {
  it('per-product token endpoints are retired; every product uses mgmtTokenEndpoint', () => {
    const config = ConfigSchema.parse({
      ...CREDS,
      mgmtTokenEndpoint: 'https://shared.example/token',
      redTeamTokenEndpoint: 'https://retired.example',
      aiGwTokenEndpoint: 'https://retired.example',
    });
    for (const build of [redTeamClientOptions, aiGatewayClientOptions, agentGuardClientOptions])
      expect(build(config).tokenEndpoint).toBe('https://shared.example/token');
  });

  it('applies product base-URL overrides from the file', () => {
    const config = ConfigSchema.parse({
      ...CREDS,
      dlpEndpoint: 'https://dlp.example',
      redTeamDataEndpoint: 'https://rt.example',
      modelSecMgmtEndpoint: 'https://ms.example',
      agentGuardDataEndpoint: 'https://ag.example',
      aiGwAdminEndpoint: 'https://gw.example',
      iamEndpoint: 'https://iam.example',
    });
    expect(managementClientOptions(config).dlpEndpoint).toBe('https://dlp.example');
    expect(redTeamClientOptions(config).dataEndpoint).toBe('https://rt.example');
    expect(modelSecurityClientOptions(config).mgmtEndpoint).toBe('https://ms.example');
    expect(agentGuardClientOptions(config).dataEndpoint).toBe('https://ag.example');
    expect(aiGatewayClientOptions(config).adminEndpoint).toBe('https://gw.example');
    expect(aiGatewayClientOptions(config).iamEndpoint).toBe('https://iam.example');
  });

  it('managementClientOptions', () => {
    expect(managementClientOptions(ConfigSchema.parse(CREDS))).toEqual({
      ...OAUTH,
      tokenEndpoint: DEFAULT_TOKEN_ENDPOINT,
      apiEndpoint: DEFAULT_MGMT_ENDPOINT,
      dashboardEndpoint: DEFAULT_MGMT_DASHBOARD_ENDPOINT,
      dlpEndpoint: DEFAULT_DLP_ENDPOINT,
    });
    expect(
      managementClientOptions(
        ConfigSchema.parse({
          ...CREDS,
          mgmtEndpoint: 'https://management.example',
          mgmtDashboardEndpoint: 'https://dashboard.example/aisec',
        }),
      ),
    ).toMatchObject({
      apiEndpoint: 'https://management.example',
      dashboardEndpoint: 'https://dashboard.example/aisec',
      dlpEndpoint: DEFAULT_DLP_ENDPOINT,
    });
  });

  it('redTeamClientOptions', () => {
    expect(redTeamClientOptions(ConfigSchema.parse(CREDS))).toEqual({
      ...OAUTH,
      tokenEndpoint: DEFAULT_TOKEN_ENDPOINT,
      dataEndpoint: DEFAULT_RED_TEAM_DATA_ENDPOINT,
      mgmtEndpoint: DEFAULT_RED_TEAM_MGMT_ENDPOINT,
      networkBrokerEndpoint: DEFAULT_RED_TEAM_NETWORK_BROKER_ENDPOINT,
    });
  });

  it('modelSecurityClientOptions', () => {
    expect(modelSecurityClientOptions(ConfigSchema.parse(CREDS))).toEqual({
      ...OAUTH,
      tokenEndpoint: DEFAULT_TOKEN_ENDPOINT,
      dataEndpoint: DEFAULT_MODEL_SEC_DATA_ENDPOINT,
      mgmtEndpoint: DEFAULT_MODEL_SEC_MGMT_ENDPOINT,
    });
  });

  it('aiGatewayClientOptions', () => {
    expect(aiGatewayClientOptions(ConfigSchema.parse(CREDS))).toEqual({
      ...OAUTH,
      tokenEndpoint: DEFAULT_TOKEN_ENDPOINT,
      dataEndpoint: DEFAULT_AI_GW_DATA_ENDPOINT,
      adminEndpoint: DEFAULT_AI_GW_ADMIN_ENDPOINT,
      iamEndpoint: DEFAULT_IAM_ENDPOINT,
    });
  });

  it('agentGuardClientOptions', () => {
    expect(agentGuardClientOptions(ConfigSchema.parse(CREDS))).toEqual({
      ...OAUTH,
      tokenEndpoint: DEFAULT_TOKEN_ENDPOINT,
      dataEndpoint: AGENT_GUARD_DATA_ENDPOINT,
      mgmtEndpoint: AGENT_GUARD_MGMT_ENDPOINT,
    });
  });
});

describe('runtimeInitOptions', () => {
  it('maps airs* keys to SDK init options with an explicit endpoint', () => {
    const config = ConfigSchema.parse({
      airsApiKey: 'key-1',
      airsApiToken: 'tok-1',
      airsApiEndpoint: 'https://airs.example.com',
      airsNumRetries: 3,
    });
    expect(runtimeInitOptions(config)).toEqual({
      apiKey: 'key-1',
      apiToken: 'tok-1',
      apiEndpoint: 'https://airs.example.com',
      numRetries: 3,
    });
    expect(runtimeInitOptions(ConfigSchema.parse({ airsApiToken: 'tok' })).apiEndpoint).toBe(
      DEFAULT_ENDPOINT,
    );
  });

  it('refuses to init without a scanner credential even when the environment has one', () => {
    vi.stubEnv('PANW_AI_SEC_API_KEY', 'legacy');
    expect(() => runtimeInitOptions(ConfigSchema.parse({}))).toThrow(
      'Scanner credentials are not configured',
    );
  });
});
