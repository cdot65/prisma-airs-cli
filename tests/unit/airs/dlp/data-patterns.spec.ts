import { beforeEach, describe, expect, it, vi } from 'vitest';

const mocks = {
  list: vi.fn(),
  create: vi.fn(),
  get: vi.fn(),
  replace: vi.fn(),
  patch: vi.fn(),
  delete: vi.fn(),
};

vi.mock('@cdot65/prisma-airs-sdk', () => ({
  ManagementClient: vi.fn().mockImplementation(() => ({
    dlp: { dataPatterns: mocks },
  })),
}));

beforeEach(() => {
  for (const fn of Object.values(mocks)) fn.mockReset();
});

async function freshService() {
  const { _resetManagementClient } = await import('../../../../src/airs/management.js');
  _resetManagementClient();
  const { SdkDataPatternsService } = await import('../../../../src/airs/dlp/data-patterns.js');
  return new SdkDataPatternsService();
}

describe('SdkDataPatternsService', () => {
  it('list passes params', async () => {
    mocks.list.mockResolvedValue({ content: [], totalElements: 0 });
    const svc = await freshService();
    await svc.list({ page: 0, size: 50 });
    expect(mocks.list).toHaveBeenCalledWith({ page: 0, size: 50 });
  });

  it('create passes body', async () => {
    mocks.create.mockResolvedValue({ id: 'p1' });
    const svc = await freshService();
    // biome-ignore lint/suspicious/noExplicitAny: test fixture body shape not exercised
    await svc.create({ name: 'p', detection_method: { type: 'regex', regex: '.*' } } as any);
    expect(mocks.create).toHaveBeenCalled();
  });

  it('get by id', async () => {
    mocks.get.mockResolvedValue({ id: 'p1' });
    const svc = await freshService();
    expect((await svc.get('p1')).id).toBe('p1');
  });

  it('replace by id+body', async () => {
    mocks.replace.mockResolvedValue({ id: 'p1' });
    const svc = await freshService();
    // biome-ignore lint/suspicious/noExplicitAny: test fixture body shape not exercised
    await svc.replace('p1', { name: 'p' } as any);
    expect(mocks.replace).toHaveBeenCalledWith('p1', { name: 'p' });
  });

  it('patch passes raw merge-patch body', async () => {
    mocks.patch.mockResolvedValue({ id: 'p1' });
    const svc = await freshService();
    // biome-ignore lint/suspicious/noExplicitAny: test fixture body shape not exercised
    await svc.patch('p1', { name: 'new', description: null } as any);
    expect(mocks.patch).toHaveBeenCalledWith('p1', { name: 'new', description: null });
  });

  it('delete returns void', async () => {
    mocks.delete.mockResolvedValue(undefined);
    const svc = await freshService();
    await expect(svc.delete('p1')).resolves.toBeUndefined();
    expect(mocks.delete).toHaveBeenCalledWith('p1');
  });
});
