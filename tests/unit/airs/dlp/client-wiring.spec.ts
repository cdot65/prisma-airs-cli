import { DEFAULT_DLP_ENDPOINT } from '@cdot65/prisma-airs-sdk';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { managementClientOptions } from '../../../../src/config/client-options.js';
import { ConfigSchema } from '../../../../src/config/schema.js';

const mockMgmtCtor = vi.fn();
vi.mock('@cdot65/prisma-airs-sdk', async (importOriginal) => ({
  ...(await importOriginal<typeof import('@cdot65/prisma-airs-sdk')>()),
  ManagementClient: vi.fn().mockImplementation((opts) => {
    mockMgmtCtor(opts);
    return {
      dlp: { dataFilteringProfiles: {}, dataPatterns: {}, dataProfiles: {}, dictionaries: {} },
    };
  }),
}));

beforeEach(async () => {
  mockMgmtCtor.mockReset();
  const { _resetManagementClient } = await import('../../../../src/airs/management.js');
  _resetManagementClient();
});

describe('DLP client wiring', () => {
  it('passes the SDK default DLP endpoint so no environment override applies', async () => {
    vi.stubEnv('PANW_DLP_ENDPOINT', 'https://legacy.example');
    const { SdkManagementService } = await import('../../../../src/airs/management.js');
    const creds = { mgmtClientId: 'c', mgmtClientSecret: 's', mgmtTsgId: '1' };
    new SdkManagementService(managementClientOptions(ConfigSchema.parse(creds)));
    expect(mockMgmtCtor).toHaveBeenCalledWith(
      expect.objectContaining({ dlpEndpoint: DEFAULT_DLP_ENDPOINT }),
    );
    vi.unstubAllEnvs();
  });

  it('honors a dlpEndpoint override from the tenant file', async () => {
    const { _resetManagementClient, SdkManagementService } = await import(
      '../../../../src/airs/management.js'
    );
    _resetManagementClient();
    const config = ConfigSchema.parse({
      mgmtClientId: 'c',
      mgmtClientSecret: 's',
      mgmtTsgId: '1',
      dlpEndpoint: 'https://dlp.example',
    });
    new SdkManagementService(managementClientOptions(config));
    expect(mockMgmtCtor).toHaveBeenCalledWith(
      expect.objectContaining({ dlpEndpoint: 'https://dlp.example' }),
    );
  });
});
