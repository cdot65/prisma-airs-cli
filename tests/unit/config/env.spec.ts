import { describe, expect, it } from 'vitest';
import {
  ignoredEnvironment,
  REGISTRY_ENV_VARS,
  SDK_DIAGNOSTIC_ENV_VARS,
} from '../../../src/config/env.js';

describe('ignoredEnvironment', () => {
  it('reports every set PANW_* and retired name, sorted, except SDK diagnostics and registry vars', () => {
    expect(
      ignoredEnvironment({
        PANW_MGMT_CLIENT_SECRET: 'x',
        PANW_AI_SEC_API_KEY: 'x',
        PANW_CLI_OUTPUT: 'json',
        PANW_RED_TEAM_CLIENT_ID: 'x',
        PRISMA_AIRS_CONFIG_PATH: '/x',
        SCAN_CONCURRENCY: '3',
        DATA_DIR: '',
        PANW_AI_SEC_DEBUG: '1',
        PANW_AI_SEC_TIMEOUT_MS: '1',
        PRISMA_AIRS_TENANTS_PATH: '/r',
        XDG_STATE_HOME: '/s',
        HOME: '/h',
      }),
    ).toEqual([
      'PANW_AI_SEC_API_KEY',
      'PANW_CLI_OUTPUT',
      'PANW_MGMT_CLIENT_SECRET',
      'PANW_RED_TEAM_CLIENT_ID',
      'PRISMA_AIRS_CONFIG_PATH',
      'SCAN_CONCURRENCY',
    ]);
  });

  it('is empty for a clean environment', () => {
    expect(ignoredEnvironment({ HOME: '/h', PATH: '/bin' })).toEqual([]);
  });

  it('documents the only variables the CLI or SDK still honor', () => {
    expect([...SDK_DIAGNOSTIC_ENV_VARS]).toEqual([
      'PANW_AI_SEC_DEBUG',
      'PANW_AI_SEC_DEBUG_BODY',
      'PANW_AI_SEC_TIMEOUT_MS',
    ]);
    expect([...REGISTRY_ENV_VARS]).toEqual(['PRISMA_AIRS_TENANTS_PATH', 'XDG_STATE_HOME']);
  });
});
