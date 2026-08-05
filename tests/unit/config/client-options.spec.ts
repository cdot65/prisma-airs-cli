import { describe, expect, it } from 'vitest';
import {
  aiGatewayClientOptions,
  modelSecurityClientOptions,
  redTeamClientOptions,
  runtimeInitOptions,
} from '../../../src/config/client-options.js';
import { ConfigSchema } from '../../../src/config/schema.js';

describe('runtimeInitOptions', () => {
  it('maps airs* keys to SDK init options', () => {
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
  });

  it('passes undefined for unset keys (SDK falls back to env/defaults)', () => {
    const config = ConfigSchema.parse({ airsApiKey: 'key-1' });
    expect(runtimeInitOptions(config)).toEqual({
      apiKey: 'key-1',
      apiToken: undefined,
      apiEndpoint: undefined,
      numRetries: undefined,
    });
  });
});

describe('redTeamClientOptions', () => {
  it('maps mgmt creds and redTeam* endpoints', () => {
    const config = ConfigSchema.parse({
      mgmtClientId: 'cid',
      mgmtClientSecret: 'sec',
      mgmtTsgId: 'tsg',
      redTeamDataEndpoint: 'https://rt-data.example.com',
      redTeamMgmtEndpoint: 'https://rt-mgmt.example.com',
      redTeamTokenEndpoint: 'https://rt-token.example.com',
    });
    expect(redTeamClientOptions(config)).toEqual({
      clientId: 'cid',
      clientSecret: 'sec',
      tsgId: 'tsg',
      dataEndpoint: 'https://rt-data.example.com',
      mgmtEndpoint: 'https://rt-mgmt.example.com',
      tokenEndpoint: 'https://rt-token.example.com',
    });
  });

  it('falls back to mgmtTokenEndpoint when redTeamTokenEndpoint unset', () => {
    const config = ConfigSchema.parse({
      mgmtTokenEndpoint: 'https://mgmt-token.example.com',
    });
    expect(redTeamClientOptions(config).tokenEndpoint).toBe('https://mgmt-token.example.com');
  });

  it('prefers redTeamTokenEndpoint over mgmtTokenEndpoint', () => {
    const config = ConfigSchema.parse({
      mgmtTokenEndpoint: 'https://mgmt-token.example.com',
      redTeamTokenEndpoint: 'https://rt-token.example.com',
    });
    expect(redTeamClientOptions(config).tokenEndpoint).toBe('https://rt-token.example.com');
  });
});

describe('modelSecurityClientOptions', () => {
  it('maps mgmt creds and modelSec* endpoints', () => {
    const config = ConfigSchema.parse({
      mgmtClientId: 'cid',
      mgmtClientSecret: 'sec',
      mgmtTsgId: 'tsg',
      modelSecDataEndpoint: 'https://ms-data.example.com',
      modelSecMgmtEndpoint: 'https://ms-mgmt.example.com',
      modelSecTokenEndpoint: 'https://ms-token.example.com',
    });
    expect(modelSecurityClientOptions(config)).toEqual({
      clientId: 'cid',
      clientSecret: 'sec',
      tsgId: 'tsg',
      dataEndpoint: 'https://ms-data.example.com',
      mgmtEndpoint: 'https://ms-mgmt.example.com',
      tokenEndpoint: 'https://ms-token.example.com',
    });
  });

  it('falls back to mgmtTokenEndpoint when modelSecTokenEndpoint unset', () => {
    const config = ConfigSchema.parse({
      mgmtTokenEndpoint: 'https://mgmt-token.example.com',
    });
    expect(modelSecurityClientOptions(config).tokenEndpoint).toBe('https://mgmt-token.example.com');
  });
});

describe('aiGatewayClientOptions', () => {
  it('maps mgmt creds and aiGw* endpoints', () => {
    const config = ConfigSchema.parse({
      mgmtClientId: 'cid',
      mgmtClientSecret: 'sec',
      mgmtTsgId: 'tsg',
      aiGwDataEndpoint: 'https://gw-data.example.com/ai_gw/v2',
      aiGwAdminEndpoint: 'https://gw-admin.example.com/ai_gw/admin/v2',
      aiGwTokenEndpoint: 'https://gw-token.example.com',
    });
    expect(aiGatewayClientOptions(config)).toEqual({
      clientId: 'cid',
      clientSecret: 'sec',
      tsgId: 'tsg',
      dataEndpoint: 'https://gw-data.example.com/ai_gw/v2',
      adminEndpoint: 'https://gw-admin.example.com/ai_gw/admin/v2',
      tokenEndpoint: 'https://gw-token.example.com',
    });
  });

  it('falls back to mgmtTokenEndpoint when aiGwTokenEndpoint unset', () => {
    const config = ConfigSchema.parse({
      mgmtTokenEndpoint: 'https://mgmt-token.example.com',
    });
    expect(aiGatewayClientOptions(config).tokenEndpoint).toBe('https://mgmt-token.example.com');
  });

  it('prefers aiGwTokenEndpoint over mgmtTokenEndpoint', () => {
    const config = ConfigSchema.parse({
      mgmtTokenEndpoint: 'https://mgmt-token.example.com',
      aiGwTokenEndpoint: 'https://gw-token.example.com',
    });
    expect(aiGatewayClientOptions(config).tokenEndpoint).toBe('https://gw-token.example.com');
  });
});
