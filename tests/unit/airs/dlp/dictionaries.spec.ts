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
    dlp: { dictionaries: mocks },
  })),
}));

beforeEach(() => {
  for (const fn of Object.values(mocks)) fn.mockReset();
});

async function freshService() {
  const { _resetManagementClient } = await import('../../../../src/airs/management.js');
  _resetManagementClient();
  const { SdkDictionariesService } = await import('../../../../src/airs/dlp/dictionaries.js');
  return new SdkDictionariesService();
}

describe('SdkDictionariesService', () => {
  it('list passes keywords flag', async () => {
    mocks.list.mockResolvedValue({ content: [], totalElements: 0 });
    await (await freshService()).list({ page: 0, keywords: true });
    expect(mocks.list).toHaveBeenCalledWith({ page: 0, keywords: true });
  });

  it('create passes metadata + file + includeKeywords through', async () => {
    mocks.create.mockResolvedValue({ id: 'd1' });
    const buf = Buffer.from('word1\nword2\n');
    await (await freshService()).create({
      // biome-ignore lint/suspicious/noExplicitAny: test fixture metadata shape not exercised
      metadata: { name: 'd', region: 'us', category: 'misc', original_file_name: 'd.txt' } as any,
      file: buf,
      includeKeywords: true,
    });
    expect(mocks.create).toHaveBeenCalledWith(
      expect.objectContaining({ file: buf, includeKeywords: true }),
    );
  });

  it('get with --include-keywords', async () => {
    mocks.get.mockResolvedValue({ id: 'd1' });
    await (await freshService()).get('d1', { includeKeywords: true });
    expect(mocks.get).toHaveBeenCalledWith('d1', { includeKeywords: true });
  });

  it('replace returns 200 body verbatim', async () => {
    mocks.replace.mockResolvedValue({ id: 'd1', name: 'x' });
    const r = await (await freshService()).replace('d1', {
      // biome-ignore lint/suspicious/noExplicitAny: test fixture metadata shape not exercised
      metadata: { name: 'x', region: 'us', category: 'misc', original_file_name: 'd.txt' } as any,
      file: Buffer.from('a'),
    });
    expect(r).toEqual({ id: 'd1', name: 'x' });
  });

  it('replace 204 → re-gets and returns get result', async () => {
    mocks.replace.mockResolvedValue(undefined);
    mocks.get.mockResolvedValue({ id: 'd1', name: 'after-204' });
    const r = await (await freshService()).replace('d1', {
      // biome-ignore lint/suspicious/noExplicitAny: test fixture metadata shape not exercised
      metadata: { name: 'x', region: 'us', category: 'misc', original_file_name: 'd.txt' } as any,
      file: Buffer.from('a'),
    });
    expect(mocks.get).toHaveBeenCalledWith('d1');
    expect(r).toEqual({ id: 'd1', name: 'after-204' });
  });

  it('replace 204 + re-get failure → returns fallback sentinel', async () => {
    mocks.replace.mockResolvedValue(undefined);
    mocks.get.mockRejectedValue(new Error('transient 503'));
    const r = await (await freshService()).replace('d1', {
      // biome-ignore lint/suspicious/noExplicitAny: test fixture metadata shape not exercised
      metadata: { name: 'x', region: 'us', category: 'misc', original_file_name: 'd.txt' } as any,
      file: Buffer.from('a'),
    });
    expect(r).toEqual({ kind: 'fallback', id: 'd1' });
  });

  it('patch passes body', async () => {
    mocks.patch.mockResolvedValue({ id: 'd1' });
    // biome-ignore lint/suspicious/noExplicitAny: test fixture body shape not exercised
    await (await freshService()).patch('d1', { description: 'new' } as any);
    expect(mocks.patch).toHaveBeenCalledWith('d1', { description: 'new' });
  });

  it('delete returns void', async () => {
    mocks.delete.mockResolvedValue(undefined);
    await expect((await freshService()).delete('d1')).resolves.toBeUndefined();
    expect(mocks.delete).toHaveBeenCalledWith('d1');
  });
});
