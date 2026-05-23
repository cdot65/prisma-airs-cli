import { beforeEach, describe, expect, it, vi } from 'vitest';

const mockList = vi.fn();
const mockGet = vi.fn();
const mockReplace = vi.fn();

vi.mock('@cdot65/prisma-airs-sdk', () => ({
  ManagementClient: vi.fn().mockImplementation(() => ({
    dlp: {
      dataFilteringProfiles: { list: mockList, get: mockGet, replace: mockReplace },
    },
  })),
}));

beforeEach(() => {
  mockList.mockReset();
  mockGet.mockReset();
  mockReplace.mockReset();
});

describe('SdkDataFilteringProfilesService', () => {
  it('list passes page/size/sort through', async () => {
    mockList.mockResolvedValue({ content: [], totalElements: 0 });
    const { SdkDataFilteringProfilesService } = await import(
      '../../../../src/airs/dlp/data-filtering-profiles.js'
    );
    const { _resetManagementClient } = await import('../../../../src/airs/management.js');
    _resetManagementClient();
    const svc = new SdkDataFilteringProfilesService();
    await svc.list({ page: 1, size: 25, sort: ['name,asc', 'createdAt,desc'] });
    expect(mockList).toHaveBeenCalledWith({
      page: 1,
      size: 25,
      sort: ['name,asc', 'createdAt,desc'],
    });
  });

  it('list handles empty Page envelope', async () => {
    mockList.mockResolvedValue({ content: [], totalElements: 0, pageable: { pageNumber: 0 } });
    const { SdkDataFilteringProfilesService } = await import(
      '../../../../src/airs/dlp/data-filtering-profiles.js'
    );
    const { _resetManagementClient } = await import('../../../../src/airs/management.js');
    _resetManagementClient();
    const svc = new SdkDataFilteringProfilesService();
    const r = await svc.list();
    expect(r.totalElements).toBe(0);
  });

  it('get round-trips id', async () => {
    mockGet.mockResolvedValue({ id: 'abc', name: 'x' });
    const { SdkDataFilteringProfilesService } = await import(
      '../../../../src/airs/dlp/data-filtering-profiles.js'
    );
    const { _resetManagementClient } = await import('../../../../src/airs/management.js');
    _resetManagementClient();
    const svc = new SdkDataFilteringProfilesService();
    const r = await svc.get('abc');
    expect(mockGet).toHaveBeenCalledWith('abc');
    expect(r.id).toBe('abc');
  });

  it('replace passes body through', async () => {
    mockReplace.mockResolvedValue({ id: 'abc' });
    // biome-ignore lint/suspicious/noExplicitAny: test fixture body shape not exercised
    const body = { name: 'p' } as any;
    const { SdkDataFilteringProfilesService } = await import(
      '../../../../src/airs/dlp/data-filtering-profiles.js'
    );
    const { _resetManagementClient } = await import('../../../../src/airs/management.js');
    _resetManagementClient();
    const svc = new SdkDataFilteringProfilesService();
    await svc.replace('abc', body);
    expect(mockReplace).toHaveBeenCalledWith('abc', body);
  });
});
