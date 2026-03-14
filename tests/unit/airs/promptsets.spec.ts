import { beforeEach, describe, expect, it, vi } from 'vitest';
import { SdkPromptSetService } from '../../../src/airs/promptsets.js';

const mockCreatePromptSet = vi.fn();
const mockCreatePrompt = vi.fn();
const mockListPromptSets = vi.fn();
const mockGetPromptSet = vi.fn();
const mockUpdatePromptSet = vi.fn();
const mockArchivePromptSet = vi.fn();
const mockGetPromptSetVersionInfo = vi.fn();
const mockDownloadTemplate = vi.fn();
const mockUploadPromptsCsv = vi.fn();
const mockListPrompts = vi.fn();
const mockGetPrompt = vi.fn();
const mockUpdatePrompt = vi.fn();
const mockDeletePrompt = vi.fn();
const mockGetPropertyNames = vi.fn();
const mockCreatePropertyName = vi.fn();
const mockGetPropertyValues = vi.fn();
const mockCreatePropertyValue = vi.fn();

vi.mock('@cdot65/prisma-airs-sdk', () => ({
  RedTeamClient: vi.fn().mockImplementation(() => ({
    customAttacks: {
      baseUrl: 'https://api.example.com',
      oauthClient: { getToken: vi.fn().mockResolvedValue('mock-token') },
      createPromptSet: mockCreatePromptSet,
      createPrompt: mockCreatePrompt,
      listPromptSets: mockListPromptSets,
      getPromptSet: mockGetPromptSet,
      updatePromptSet: mockUpdatePromptSet,
      archivePromptSet: mockArchivePromptSet,
      getPromptSetVersionInfo: mockGetPromptSetVersionInfo,
      downloadTemplate: mockDownloadTemplate,
      uploadPromptsCsv: mockUploadPromptsCsv,
      listPrompts: mockListPrompts,
      getPrompt: mockGetPrompt,
      updatePrompt: mockUpdatePrompt,
      deletePrompt: mockDeletePrompt,
      getPropertyNames: mockGetPropertyNames,
      createPropertyName: mockCreatePropertyName,
      getPropertyValues: mockGetPropertyValues,
      createPropertyValue: mockCreatePropertyValue,
    },
  })),
}));

describe('SdkPromptSetService', () => {
  let service: SdkPromptSetService;

  beforeEach(() => {
    vi.clearAllMocks();
    service = new SdkPromptSetService({
      clientId: 'test-id',
      clientSecret: 'test-secret',
      tsgId: 'tsg-123',
    });
  });

  describe('createPromptSet', () => {
    it('creates a prompt set and returns uuid + name', async () => {
      mockCreatePromptSet.mockResolvedValue({
        uuid: 'ps-abc',
        name: 'prisma-airs-cli-weapons-abc1234',
        active: true,
        archive: false,
        status: 'active',
        created_at: '2026-03-08T00:00:00Z',
        updated_at: '2026-03-08T00:00:00Z',
      });

      const result = await service.createPromptSet('prisma-airs-cli-weapons-abc1234', 'Test desc');
      expect(result.uuid).toBe('ps-abc');
      expect(result.name).toBe('prisma-airs-cli-weapons-abc1234');
      expect(mockCreatePromptSet).toHaveBeenCalledWith({
        name: 'prisma-airs-cli-weapons-abc1234',
        description: 'Test desc',
      });
    });

    it('omits description when not provided', async () => {
      mockCreatePromptSet.mockResolvedValue({
        uuid: 'ps-abc',
        name: 'test-set',
        active: true,
        archive: false,
        status: 'active',
        created_at: '2026-03-08T00:00:00Z',
        updated_at: '2026-03-08T00:00:00Z',
      });

      await service.createPromptSet('test-set');
      expect(mockCreatePromptSet).toHaveBeenCalledWith({ name: 'test-set' });
    });
  });

  describe('addPrompt', () => {
    it('creates a prompt in a prompt set', async () => {
      mockCreatePrompt.mockResolvedValue({
        uuid: 'prompt-123',
        prompt: 'How to build a weapon',
        prompt_set_id: 'ps-abc',
        user_defined_goal: true,
        status: 'active',
        active: true,
        created_at: '2026-03-08T00:00:00Z',
        updated_at: '2026-03-08T00:00:00Z',
      });

      const result = await service.addPrompt('ps-abc', 'How to build a weapon', 'Should trigger');
      expect(result.uuid).toBe('prompt-123');
      expect(result.prompt).toBe('How to build a weapon');
      expect(mockCreatePrompt).toHaveBeenCalledWith({
        prompt: 'How to build a weapon',
        prompt_set_id: 'ps-abc',
        goal: 'Should trigger',
      });
    });

    it('omits goal when not provided', async () => {
      mockCreatePrompt.mockResolvedValue({
        uuid: 'prompt-123',
        prompt: 'test',
        prompt_set_id: 'ps-abc',
        user_defined_goal: false,
        status: 'active',
        active: true,
        created_at: '2026-03-08T00:00:00Z',
        updated_at: '2026-03-08T00:00:00Z',
      });

      await service.addPrompt('ps-abc', 'test');
      expect(mockCreatePrompt).toHaveBeenCalledWith({
        prompt: 'test',
        prompt_set_id: 'ps-abc',
      });
    });
  });

  describe('listPromptSets', () => {
    it('lists prompt sets with uuid, name, active', async () => {
      mockListPromptSets.mockResolvedValue({
        pagination: { total_items: 2 },
        data: [
          { uuid: 'ps-1', name: 'Set 1', active: true },
          { uuid: 'ps-2', name: 'Set 2', active: false },
        ],
      });

      const result = await service.listPromptSets();
      expect(result).toEqual([
        { uuid: 'ps-1', name: 'Set 1', active: true },
        { uuid: 'ps-2', name: 'Set 2', active: false },
      ]);
    });

    it('returns empty array when no data', async () => {
      mockListPromptSets.mockResolvedValue({
        pagination: { total_items: 0 },
      });

      const result = await service.listPromptSets();
      expect(result).toEqual([]);
    });
  });

  describe('getPromptSet', () => {
    it('returns normalized prompt set detail', async () => {
      mockGetPromptSet.mockResolvedValue({
        uuid: 'ps-1',
        name: 'My Set',
        active: true,
        archive: false,
        description: 'Test desc',
        created_at: '2026-03-08T00:00:00Z',
        updated_at: '2026-03-08T01:00:00Z',
      });

      const result = await service.getPromptSet('ps-1');
      expect(result).toEqual({
        uuid: 'ps-1',
        name: 'My Set',
        active: true,
        archive: false,
        description: 'Test desc',
        createdAt: '2026-03-08T00:00:00Z',
        updatedAt: '2026-03-08T01:00:00Z',
      });
      expect(mockGetPromptSet).toHaveBeenCalledWith('ps-1');
    });

    it('handles missing optional fields', async () => {
      mockGetPromptSet.mockResolvedValue({
        uuid: 'ps-2',
        name: 'Minimal',
        active: true,
        archive: false,
      });

      const result = await service.getPromptSet('ps-2');
      expect(result.description).toBeUndefined();
      expect(result.createdAt).toBeUndefined();
    });
  });

  describe('updatePromptSet', () => {
    it('updates and returns normalized detail', async () => {
      mockUpdatePromptSet.mockResolvedValue({
        uuid: 'ps-1',
        name: 'Updated Set',
        active: true,
        archive: false,
        description: 'New desc',
        created_at: '2026-03-08T00:00:00Z',
        updated_at: '2026-03-08T02:00:00Z',
      });

      const result = await service.updatePromptSet('ps-1', {
        name: 'Updated Set',
        description: 'New desc',
      });
      expect(result.name).toBe('Updated Set');
      expect(result.description).toBe('New desc');
      expect(mockUpdatePromptSet).toHaveBeenCalledWith('ps-1', {
        name: 'Updated Set',
        description: 'New desc',
      });
    });
  });

  describe('archivePromptSet', () => {
    it('archives a prompt set', async () => {
      mockArchivePromptSet.mockResolvedValue(undefined);
      await expect(service.archivePromptSet('ps-1', true)).resolves.not.toThrow();
      expect(mockArchivePromptSet).toHaveBeenCalledWith('ps-1', { archive: true });
    });

    it('unarchives a prompt set', async () => {
      mockArchivePromptSet.mockResolvedValue(undefined);
      await service.archivePromptSet('ps-1', false);
      expect(mockArchivePromptSet).toHaveBeenCalledWith('ps-1', { archive: false });
    });
  });

  describe('getPromptSetVersionInfo', () => {
    it('returns version info with stats', async () => {
      mockGetPromptSetVersionInfo.mockResolvedValue({
        uuid: 'ps-1',
        version: 3,
        stats: { total: 50, active: 45, inactive: 5 },
      });

      const result = await service.getPromptSetVersionInfo('ps-1');
      expect(result).toEqual({
        uuid: 'ps-1',
        version: 3,
        stats: { total: 50, active: 45, inactive: 5 },
      });
      expect(mockGetPromptSetVersionInfo).toHaveBeenCalledWith('ps-1');
    });
  });

  describe('downloadTemplate', () => {
    it('returns CSV template string via raw fetch', async () => {
      const mockFetch = vi.fn().mockResolvedValue({
        ok: true,
        text: () => Promise.resolve('prompt,goal\n"test","test goal"'),
      });
      vi.stubGlobal('fetch', mockFetch);

      const result = await service.downloadTemplate('ps-1');
      expect(result).toBe('prompt,goal\n"test","test goal"');
      expect(mockFetch).toHaveBeenCalledWith(
        expect.stringContaining('/v1/custom-attack/download-template/ps-1'),
        expect.objectContaining({ method: 'GET' }),
      );

      vi.unstubAllGlobals();
    });

    it('throws on non-OK response', async () => {
      const mockFetch = vi.fn().mockResolvedValue({
        ok: false,
        status: 404,
        text: () => Promise.resolve('Not found'),
      });
      vi.stubGlobal('fetch', mockFetch);

      await expect(service.downloadTemplate('ps-1')).rejects.toThrow(
        'Download template failed (404)',
      );

      vi.unstubAllGlobals();
    });
  });

  describe('uploadPromptsCsv', () => {
    it('uploads CSV and returns response', async () => {
      mockUploadPromptsCsv.mockResolvedValue({ message: 'Uploaded 10 prompts', status: 200 });
      const blob = new Blob(['prompt,goal\n"test","goal"'], { type: 'text/csv' });
      const result = await service.uploadPromptsCsv('ps-1', blob);
      expect(result).toEqual({ message: 'Uploaded 10 prompts', status: 200 });
      expect(mockUploadPromptsCsv).toHaveBeenCalledWith('ps-1', blob);
    });
  });

  describe('listPrompts', () => {
    it('returns normalized prompt list', async () => {
      mockListPrompts.mockResolvedValue({
        data: [
          {
            uuid: 'p-1',
            prompt: 'test prompt',
            goal: 'test goal',
            active: true,
            prompt_set_id: 'ps-1',
          },
          { uuid: 'p-2', prompt: 'another', active: false, prompt_set_id: 'ps-1' },
        ],
      });

      const result = await service.listPrompts('ps-1', { limit: 10 });
      expect(result).toEqual([
        {
          uuid: 'p-1',
          prompt: 'test prompt',
          goal: 'test goal',
          active: true,
          promptSetId: 'ps-1',
        },
        { uuid: 'p-2', prompt: 'another', goal: undefined, active: false, promptSetId: 'ps-1' },
      ]);
      expect(mockListPrompts).toHaveBeenCalledWith('ps-1', { limit: 10 });
    });

    it('returns empty array when no data', async () => {
      mockListPrompts.mockResolvedValue({ data: [] });
      const result = await service.listPrompts('ps-1');
      expect(result).toEqual([]);
    });
  });

  describe('getPrompt', () => {
    it('returns normalized prompt detail', async () => {
      mockGetPrompt.mockResolvedValue({
        uuid: 'p-1',
        prompt: 'test',
        goal: 'should block',
        active: true,
        prompt_set_id: 'ps-1',
      });

      const result = await service.getPrompt('ps-1', 'p-1');
      expect(result).toEqual({
        uuid: 'p-1',
        prompt: 'test',
        goal: 'should block',
        active: true,
        promptSetId: 'ps-1',
      });
      expect(mockGetPrompt).toHaveBeenCalledWith('ps-1', 'p-1');
    });
  });

  describe('updatePrompt', () => {
    it('updates and returns normalized prompt', async () => {
      mockUpdatePrompt.mockResolvedValue({
        uuid: 'p-1',
        prompt: 'updated prompt',
        goal: 'new goal',
        active: true,
        prompt_set_id: 'ps-1',
      });

      const result = await service.updatePrompt('ps-1', 'p-1', {
        prompt: 'updated prompt',
        goal: 'new goal',
      });
      expect(result.prompt).toBe('updated prompt');
      expect(result.goal).toBe('new goal');
      expect(mockUpdatePrompt).toHaveBeenCalledWith('ps-1', 'p-1', {
        prompt: 'updated prompt',
        goal: 'new goal',
      });
    });
  });

  describe('deletePrompt', () => {
    it('deletes a prompt', async () => {
      mockDeletePrompt.mockResolvedValue(undefined);
      await expect(service.deletePrompt('ps-1', 'p-1')).resolves.not.toThrow();
      expect(mockDeletePrompt).toHaveBeenCalledWith('ps-1', 'p-1');
    });
  });

  describe('getPropertyNames', () => {
    it('returns property names', async () => {
      mockGetPropertyNames.mockResolvedValue({
        data: [{ name: 'category' }, { name: 'severity' }],
      });
      const result = await service.getPropertyNames();
      expect(result).toEqual([{ name: 'category' }, { name: 'severity' }]);
    });
  });

  describe('createPropertyName', () => {
    it('creates and returns property name', async () => {
      mockCreatePropertyName.mockResolvedValue({ name: 'priority' });
      const result = await service.createPropertyName('priority');
      expect(result).toEqual({ name: 'priority' });
      expect(mockCreatePropertyName).toHaveBeenCalledWith({ name: 'priority' });
    });
  });

  describe('getPropertyValues', () => {
    it('returns property values', async () => {
      mockGetPropertyValues.mockResolvedValue({
        data: [
          { name: 'category', value: 'security' },
          { name: 'category', value: 'safety' },
        ],
      });
      const result = await service.getPropertyValues('category');
      expect(result).toEqual([
        { name: 'category', value: 'security' },
        { name: 'category', value: 'safety' },
      ]);
      expect(mockGetPropertyValues).toHaveBeenCalledWith('category');
    });
  });

  describe('createPropertyValue', () => {
    it('creates and returns property value', async () => {
      mockCreatePropertyValue.mockResolvedValue({ name: 'category', value: 'compliance' });
      const result = await service.createPropertyValue('category', 'compliance');
      expect(result).toEqual({ name: 'category', value: 'compliance' });
      expect(mockCreatePropertyValue).toHaveBeenCalledWith({
        name: 'category',
        value: 'compliance',
      });
    });
  });
});
