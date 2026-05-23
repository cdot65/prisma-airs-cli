import { beforeEach, describe, expect, it, vi } from 'vitest';

const mocks = {
  list: vi.fn(),
  create: vi.fn(),
  get: vi.fn(),
  replace: vi.fn(),
  patch: vi.fn(),
};

vi.mock('@cdot65/prisma-airs-sdk', () => ({
  ManagementClient: vi.fn().mockImplementation(() => ({
    dlp: { dataProfiles: mocks },
  })),
}));

beforeEach(() => {
  for (const fn of Object.values(mocks)) fn.mockReset();
});

async function freshService() {
  const { _resetManagementClient } = await import('../../../../src/airs/management.js');
  _resetManagementClient();
  const { SdkDataProfilesService } = await import('../../../../src/airs/dlp/data-profiles.js');
  return new SdkDataProfilesService();
}

describe('SdkDataProfilesService', () => {
  it('list passes params', async () => {
    mocks.list.mockResolvedValue({ content: [], totalElements: 0 });
    await (await freshService()).list({ page: 0 });
    expect(mocks.list).toHaveBeenCalledWith({ page: 0 });
  });

  it('create basic DetectionRule variant', async () => {
    mocks.create.mockResolvedValue({ id: 'dp1' });
    await (await freshService()).create({
      name: 'p',
      profile_type: 'custom',
      detection_rules: [{ type: 'basic', data_pattern_id: 'x', occurrence: { min: 1 } }],
      // biome-ignore lint/suspicious/noExplicitAny: test fixture body shape not exercised
    } as any);
    expect(mocks.create).toHaveBeenCalled();
  });

  it('create expression_tree variant', async () => {
    mocks.create.mockResolvedValue({ id: 'dp1' });
    await (await freshService()).create({
      name: 'p',
      profile_type: 'custom',
      detection_rules: [{ type: 'expression_tree', expression: { operator: 'and', operands: [] } }],
      // biome-ignore lint/suspicious/noExplicitAny: test fixture body shape not exercised
    } as any);
    expect(mocks.create).toHaveBeenCalled();
  });

  it('create multi_profile variant', async () => {
    mocks.create.mockResolvedValue({ id: 'dp1' });
    await (await freshService()).create({
      name: 'p',
      profile_type: 'custom',
      detection_rules: [{ type: 'multi_profile', profile_ids: ['a', 'b'] }],
      // biome-ignore lint/suspicious/noExplicitAny: test fixture body shape not exercised
    } as any);
    expect(mocks.create).toHaveBeenCalled();
  });

  it('get/replace/patch round-trip', async () => {
    mocks.get.mockResolvedValue({ id: 'dp1' });
    mocks.replace.mockResolvedValue({ id: 'dp1' });
    mocks.patch.mockResolvedValue({ id: 'dp1' });
    const svc = await freshService();
    expect((await svc.get('dp1')).id).toBe('dp1');
    // biome-ignore lint/suspicious/noExplicitAny: test fixture body shape not exercised
    await svc.replace('dp1', { name: 'p' } as any);
    expect(mocks.replace).toHaveBeenCalledWith('dp1', { name: 'p' });
    await svc.patch('dp1', {
      profile_status: 'deleted',
      name: 'p',
      profile_type: 'custom',
      // biome-ignore lint/suspicious/noExplicitAny: test fixture body shape not exercised
    } as any);
    expect(mocks.patch).toHaveBeenCalled();
  });
});
