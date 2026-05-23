import { beforeEach, describe, expect, it, vi } from 'vitest';

const mockMgmtCtor = vi.fn();
vi.mock('@cdot65/prisma-airs-sdk', () => ({
  ManagementClient: vi.fn().mockImplementation((opts) => {
    mockMgmtCtor(opts);
    return {
      dlp: { dataFilteringProfiles: {}, dataPatterns: {}, dataProfiles: {}, dictionaries: {} },
    };
  }),
}));

beforeEach(async () => {
  mockMgmtCtor.mockReset();
  delete process.env.PANW_DLP_ENDPOINT;
  const { _resetManagementClient } = await import('../../../../src/airs/management.js');
  _resetManagementClient();
});

describe('PANW_DLP_ENDPOINT wiring', () => {
  it('passes dlpEndpoint to ManagementClient when set', async () => {
    process.env.PANW_DLP_ENDPOINT = 'https://example.com';
    const { loadConfig } = await import('../../../../src/config/loader.js');
    const { SdkManagementService } = await import('../../../../src/airs/management.js');
    const cfg = await loadConfig();
    new SdkManagementService({ dlpEndpoint: cfg.dlpEndpoint });
    expect(mockMgmtCtor).toHaveBeenCalledWith(
      expect.objectContaining({ dlpEndpoint: 'https://example.com' }),
    );
  });

  it('treats empty string as unset (falls back to SDK default)', async () => {
    process.env.PANW_DLP_ENDPOINT = '';
    const { loadConfig } = await import('../../../../src/config/loader.js');
    const { SdkManagementService } = await import('../../../../src/airs/management.js');
    const cfg = await loadConfig();
    new SdkManagementService({ dlpEndpoint: cfg.dlpEndpoint });
    expect(mockMgmtCtor).toHaveBeenCalledWith(expect.objectContaining({ dlpEndpoint: undefined }));
  });

  it('omits dlpEndpoint when env unset', async () => {
    const { loadConfig } = await import('../../../../src/config/loader.js');
    const { SdkManagementService } = await import('../../../../src/airs/management.js');
    const cfg = await loadConfig();
    new SdkManagementService({ dlpEndpoint: cfg.dlpEndpoint });
    expect(mockMgmtCtor).toHaveBeenCalledWith(expect.objectContaining({ dlpEndpoint: undefined }));
  });
});
