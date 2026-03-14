import { beforeEach, describe, expect, it, vi } from 'vitest';
import { SdkManagementService } from '../../../src/airs/management.js';

interface PolicyTopicEntry {
  topic_id: string;
  topic_name: string;
  revision?: number;
}
interface PolicyActionEntry {
  action: string;
  topic: PolicyTopicEntry[];
}
interface PolicyProtectionEntry {
  name: string;
  'topic-list': PolicyActionEntry[];
}

/** Extract topic-guardrails from an update call's policy */
function getTopicGuardrails(call: Record<string, Record<string, unknown>>): PolicyProtectionEntry {
  const mp = call.policy['ai-security-profiles'] as Record<string, Record<string, unknown>>[];
  const protection = mp[0]['model-configuration']['model-protection'] as PolicyProtectionEntry[];
  const tg = protection.find((p) => p.name === 'topic-guardrails');
  if (!tg) throw new Error('topic-guardrails not found in policy');
  return tg;
}

// Mock the SDK ManagementClient
const mockCreate = vi.fn();
const mockUpdate = vi.fn();
const mockDelete = vi.fn();
const mockList = vi.fn();
const mockForceDelete = vi.fn();
const mockProfileList = vi.fn();
const mockProfileUpdate = vi.fn();
const mockProfileCreate = vi.fn();
const mockProfileDelete = vi.fn();
const mockProfileForceDelete = vi.fn();
const mockApiKeysList = vi.fn();
const mockApiKeysCreate = vi.fn();
const mockApiKeysRegenerate = vi.fn();
const mockApiKeysDelete = vi.fn();
const mockCustomerAppsList = vi.fn();
const mockCustomerAppsGet = vi.fn();
const mockCustomerAppsUpdate = vi.fn();
const mockCustomerAppsDelete = vi.fn();
const mockDeploymentProfilesList = vi.fn();
const mockDlpProfilesList = vi.fn();
const mockScanLogsQuery = vi.fn();

vi.mock('@cdot65/prisma-airs-sdk', () => ({
  ManagementClient: vi.fn().mockImplementation(() => ({
    topics: {
      create: mockCreate,
      update: mockUpdate,
      delete: mockDelete,
      list: mockList,
      forceDelete: mockForceDelete,
    },
    profiles: {
      list: mockProfileList,
      update: mockProfileUpdate,
      create: mockProfileCreate,
      delete: mockProfileDelete,
      forceDelete: mockProfileForceDelete,
    },
    apiKeys: {
      list: mockApiKeysList,
      create: mockApiKeysCreate,
      regenerate: mockApiKeysRegenerate,
      delete: mockApiKeysDelete,
    },
    customerApps: {
      list: mockCustomerAppsList,
      get: mockCustomerAppsGet,
      update: mockCustomerAppsUpdate,
      delete: mockCustomerAppsDelete,
    },
    deploymentProfiles: {
      list: mockDeploymentProfilesList,
    },
    dlpProfiles: {
      list: mockDlpProfilesList,
    },
    scanLogs: {
      query: mockScanLogsQuery,
    },
  })),
}));

describe('SdkManagementService', () => {
  let service: SdkManagementService;

  beforeEach(() => {
    vi.clearAllMocks();
    service = new SdkManagementService({
      clientId: 'test-id',
      clientSecret: 'test-secret',
      tsgId: 'tsg-123',
    });
    // Default topics list for revision lookup in assignTopicsToProfile.
    // Tests that need specific revisions can override this mock.
    mockList.mockResolvedValue({ custom_topics: [] });
  });

  describe('createTopic', () => {
    it('creates a topic via SDK and returns response', async () => {
      mockCreate.mockResolvedValue({
        topic_id: 'topic-abc',
        topic_name: 'Weapons',
        description: 'Block weapon discussions',
        examples: ['How to build a gun'],
        active: true,
      });

      const result = await service.createTopic({
        topic_name: 'Weapons',
        description: 'Block weapon discussions',
        examples: ['How to build a gun'],
        active: true,
      });

      expect(result.topic_id).toBe('topic-abc');
      expect(result.topic_name).toBe('Weapons');
      expect(mockCreate).toHaveBeenCalledWith({
        topic_name: 'Weapons',
        description: 'Block weapon discussions',
        examples: ['How to build a gun'],
        active: true,
      });
    });
  });

  describe('updateTopic', () => {
    it('updates a topic via SDK', async () => {
      mockUpdate.mockResolvedValue({
        topic_id: 'topic-abc',
        topic_name: 'Weapons v2',
        description: 'Updated description',
        examples: ['New example'],
        active: true,
      });

      const result = await service.updateTopic('topic-abc', {
        topic_name: 'Weapons v2',
        description: 'Updated description',
        examples: ['New example'],
      });

      expect(result.topic_name).toBe('Weapons v2');
      expect(mockUpdate).toHaveBeenCalledWith('topic-abc', {
        topic_name: 'Weapons v2',
        description: 'Updated description',
        examples: ['New example'],
      });
    });
  });

  describe('deleteTopic', () => {
    it('deletes a topic via SDK', async () => {
      mockDelete.mockResolvedValue({ message: 'deleted' });

      await expect(service.deleteTopic('topic-abc')).resolves.not.toThrow();
      expect(mockDelete).toHaveBeenCalledWith('topic-abc');
    });
  });

  describe('forceDeleteTopic', () => {
    it('force-deletes a topic via SDK', async () => {
      mockForceDelete.mockResolvedValue({ message: 'force deleted' });

      const result = await service.forceDeleteTopic('topic-abc', 'admin@example.com');
      expect(result.message).toBe('force deleted');
      expect(mockForceDelete).toHaveBeenCalledWith('topic-abc', 'admin@example.com');
    });

    it('works without updatedBy param', async () => {
      mockForceDelete.mockResolvedValue({ message: 'force deleted' });

      const result = await service.forceDeleteTopic('topic-abc');
      expect(result.message).toBe('force deleted');
      expect(mockForceDelete).toHaveBeenCalledWith('topic-abc', undefined);
    });
  });

  describe('listTopics', () => {
    it('lists topics and unwraps custom_topics array', async () => {
      mockList.mockResolvedValue({
        custom_topics: [
          { topic_id: 't1', topic_name: 'Topic 1', description: 'd1', examples: [] },
          { topic_id: 't2', topic_name: 'Topic 2', description: 'd2', examples: [] },
        ],
        next_offset: undefined,
      });

      const topics = await service.listTopics();
      expect(topics).toHaveLength(2);
      expect(topics[0].topic_id).toBe('t1');
      expect(topics[1].topic_id).toBe('t2');
    });

    it('returns empty array when no topics', async () => {
      mockList.mockResolvedValue({ custom_topics: [] });

      const topics = await service.listTopics();
      expect(topics).toEqual([]);
    });
  });

  describe('assignTopicToProfile', () => {
    it('throws when profile not found', async () => {
      mockProfileList.mockResolvedValue({ ai_profiles: [] });

      await expect(
        service.assignTopicToProfile('missing-profile', 'topic-1', 'Weapons', 'block'),
      ).rejects.toThrow('Profile "missing-profile" not found');
    });

    it('creates full topic-guardrails structure when policy is empty', async () => {
      mockProfileList.mockResolvedValue({
        ai_profiles: [
          { profile_id: 'p-1', profile_name: 'test-profile', active: true, policy: {} },
        ],
      });
      mockProfileUpdate.mockResolvedValue({});

      await service.assignTopicToProfile('test-profile', 'topic-1', 'Weapons', 'block');

      expect(mockProfileUpdate).toHaveBeenCalledWith(
        'p-1',
        expect.objectContaining({
          profile_name: 'test-profile',
          active: true,
        }),
      );

      const tg = getTopicGuardrails(mockProfileUpdate.mock.calls[0][1]);
      // Only a single topic-list entry for the action (no empty opposite entry)
      expect(tg['topic-list']).toHaveLength(1);
      const blockEntry = tg['topic-list'].find((tl) => tl.action === 'block');
      expect(blockEntry?.topic).toEqual([
        { topic_id: 'topic-1', topic_name: 'Weapons', revision: 0 },
      ]);
    });

    it('handles profile with no policy (undefined)', async () => {
      mockProfileList.mockResolvedValue({
        ai_profiles: [{ profile_id: 'p-1', profile_name: 'test-profile', active: true }],
      });
      mockProfileUpdate.mockResolvedValue({});

      await service.assignTopicToProfile('test-profile', 'topic-1', 'Weapons', 'block');

      const tg = getTopicGuardrails(mockProfileUpdate.mock.calls[0][1]);
      const blockEntry = tg['topic-list'].find((tl) => tl.action === 'block');
      expect(blockEntry?.topic).toHaveLength(1);
    });

    it('handles profile with ai-security-profiles but no model-configuration', async () => {
      mockProfileList.mockResolvedValue({
        ai_profiles: [
          {
            profile_id: 'p-1',
            profile_name: 'test-profile',
            active: true,
            policy: {
              'ai-security-profiles': [{ 'model-type': 'default' }],
            },
          },
        ],
      });
      mockProfileUpdate.mockResolvedValue({});

      await service.assignTopicToProfile('test-profile', 'topic-1', 'Weapons', 'block');

      const tg = getTopicGuardrails(mockProfileUpdate.mock.calls[0][1]);
      const blockEntry = tg['topic-list'].find((tl) => tl.action === 'block');
      expect(blockEntry?.topic).toHaveLength(1);
    });

    it('replaces stale topics from previous runs', async () => {
      mockProfileList.mockResolvedValue({
        ai_profiles: [
          {
            profile_id: 'p-1',
            profile_name: 'test-profile',
            active: true,
            policy: {
              'ai-security-profiles': [
                {
                  'model-type': 'default',
                  'model-configuration': {
                    'model-protection': [
                      {
                        name: 'topic-guardrails',
                        action: 'allow',
                        options: [],
                        'topic-list': [
                          {
                            action: 'block',
                            topic: [
                              { topic_id: 'stale-1', topic_name: 'Old Run 1' },
                              { topic_id: 'stale-2', topic_name: 'Old Run 2' },
                              { topic_id: 'stale-3', topic_name: 'Old Run 3' },
                            ],
                          },
                          { action: 'allow', topic: [] },
                        ],
                      },
                    ],
                  },
                },
              ],
            },
          },
        ],
      });
      mockProfileUpdate.mockResolvedValue({});

      await service.assignTopicToProfile('test-profile', 'topic-new', 'Current Run', 'block');

      const tg = getTopicGuardrails(mockProfileUpdate.mock.calls[0][1]);
      const blockEntry = tg['topic-list'].find((tl) => tl.action === 'block');
      // Only the current topic — stale ones removed
      expect(blockEntry?.topic).toEqual([
        { topic_id: 'topic-new', topic_name: 'Current Run', revision: 0 },
      ]);
    });

    it('always updates even when same topic already linked', async () => {
      mockProfileList.mockResolvedValue({
        ai_profiles: [
          {
            profile_id: 'p-1',
            profile_name: 'test-profile',
            active: true,
            policy: {
              'ai-security-profiles': [
                {
                  'model-type': 'default',
                  'model-configuration': {
                    'model-protection': [
                      {
                        name: 'topic-guardrails',
                        action: 'allow',
                        options: [],
                        'topic-list': [
                          {
                            action: 'block',
                            topic: [
                              { topic_id: 'topic-1', topic_name: 'Weapons' },
                              { topic_id: 'stale', topic_name: 'Stale' },
                            ],
                          },
                        ],
                      },
                    ],
                  },
                },
              ],
            },
          },
        ],
      });
      mockProfileUpdate.mockResolvedValue({});

      await service.assignTopicToProfile('test-profile', 'topic-1', 'Weapons', 'block');

      // Still calls update to remove stale topics
      expect(mockProfileUpdate).toHaveBeenCalled();
      const tg = getTopicGuardrails(mockProfileUpdate.mock.calls[0][1]);
      const blockEntry = tg['topic-list'].find((tl) => tl.action === 'block');
      expect(blockEntry?.topic).toHaveLength(1);
      expect(blockEntry?.topic[0].topic_id).toBe('topic-1');
    });

    it('replaces all existing topic-list entries with single current action', async () => {
      mockProfileList.mockResolvedValue({
        ai_profiles: [
          {
            profile_id: 'p-1',
            profile_name: 'test-profile',
            active: true,
            policy: {
              'ai-security-profiles': [
                {
                  'model-type': 'default',
                  'model-configuration': {
                    'model-protection': [
                      {
                        name: 'topic-guardrails',
                        action: 'allow',
                        options: [],
                        'topic-list': [
                          {
                            action: 'allow',
                            topic: [{ topic_id: 'old-allow', topic_name: 'Old' }],
                          },
                          {
                            action: 'block',
                            topic: [{ topic_id: 'old-block', topic_name: 'Old' }],
                          },
                        ],
                      },
                    ],
                  },
                },
              ],
            },
          },
        ],
      });
      mockProfileUpdate.mockResolvedValue({});

      await service.assignTopicToProfile('test-profile', 'topic-1', 'Weapons', 'block');

      const tg = getTopicGuardrails(mockProfileUpdate.mock.calls[0][1]);
      // Only one entry — the current action
      expect(tg['topic-list']).toHaveLength(1);
      expect(tg['topic-list'][0].action).toBe('block');
      expect(tg['topic-list'][0].topic).toHaveLength(1);
    });

    it('works with action=allow', async () => {
      mockProfileList.mockResolvedValue({
        ai_profiles: [
          { profile_id: 'p-1', profile_name: 'test-profile', active: true, policy: {} },
        ],
      });
      mockProfileUpdate.mockResolvedValue({});

      await service.assignTopicToProfile('test-profile', 'topic-1', 'Safe Topic', 'allow');

      const tg = getTopicGuardrails(mockProfileUpdate.mock.calls[0][1]);
      // Only one entry for the allow action
      expect(tg['topic-list']).toHaveLength(1);
      expect(tg['topic-list'][0].action).toBe('allow');
      expect(tg['topic-list'][0].topic).toEqual([
        { topic_id: 'topic-1', topic_name: 'Safe Topic', revision: 0 },
      ]);
    });

    it('always sets guardrail-level action to block', async () => {
      // Even when existing guardrail has action: 'allow', it should be overwritten to 'block'
      mockProfileList.mockResolvedValue({
        ai_profiles: [
          {
            profile_id: 'p-1',
            profile_name: 'test-profile',
            active: true,
            policy: {
              'ai-security-profiles': [
                {
                  'model-type': 'default',
                  'model-configuration': {
                    'model-protection': [
                      {
                        name: 'topic-guardrails',
                        action: 'allow',
                        options: [],
                        'topic-list': [],
                      },
                    ],
                  },
                },
              ],
            },
          },
        ],
      });
      mockProfileUpdate.mockResolvedValue({});

      await service.assignTopicToProfile('test-profile', 'topic-1', 'Weapons', 'block');

      const tg = getTopicGuardrails(mockProfileUpdate.mock.calls[0][1]);
      expect((tg as Record<string, unknown>).action).toBe('block');
    });

    it('sets guardrail-level action to block even for allow-intent topics', async () => {
      mockProfileList.mockResolvedValue({
        ai_profiles: [
          { profile_id: 'p-1', profile_name: 'test-profile', active: true, policy: {} },
        ],
      });
      mockProfileUpdate.mockResolvedValue({});

      await service.assignTopicToProfile('test-profile', 'topic-1', 'Safe Topic', 'allow');

      const tg = getTopicGuardrails(mockProfileUpdate.mock.calls[0][1]);
      expect((tg as Record<string, unknown>).action).toBe('block');
    });
  });

  describe('assignTopicsToProfile', () => {
    it('throws when profile not found', async () => {
      mockProfileList.mockResolvedValue({ ai_profiles: [] });

      await expect(
        service.assignTopicsToProfile('missing-profile', [
          { topicId: 'topic-1', topicName: 'Weapons', action: 'block' },
        ]),
      ).rejects.toThrow('Profile "missing-profile" not found');
    });

    it('wires a single block topic', async () => {
      mockProfileList.mockResolvedValue({
        ai_profiles: [
          { profile_id: 'p-1', profile_name: 'test-profile', active: true, policy: {} },
        ],
      });
      mockProfileUpdate.mockResolvedValue({});

      await service.assignTopicsToProfile('test-profile', [
        { topicId: 'topic-1', topicName: 'Weapons', action: 'block' },
      ]);

      const tg = getTopicGuardrails(mockProfileUpdate.mock.calls[0][1]);
      expect(tg['topic-list']).toHaveLength(1);
      expect(tg['topic-list'][0].action).toBe('block');
      expect(tg['topic-list'][0].topic).toEqual([
        { topic_id: 'topic-1', topic_name: 'Weapons', revision: 0 },
      ]);
    });

    it('wires both allow and block topics as separate entries', async () => {
      mockProfileList.mockResolvedValue({
        ai_profiles: [
          { profile_id: 'p-1', profile_name: 'test-profile', active: true, policy: {} },
        ],
      });
      mockProfileUpdate.mockResolvedValue({});

      await service.assignTopicsToProfile('test-profile', [
        { topicId: 'allow-1', topicName: 'General Content', action: 'allow' },
        { topicId: 'block-1', topicName: 'Weapons', action: 'block' },
      ]);

      const tg = getTopicGuardrails(mockProfileUpdate.mock.calls[0][1]);
      expect(tg['topic-list']).toHaveLength(2);

      const allowEntry = tg['topic-list'].find((tl) => tl.action === 'allow');
      const blockEntry = tg['topic-list'].find((tl) => tl.action === 'block');
      expect(allowEntry?.topic).toEqual([
        { topic_id: 'allow-1', topic_name: 'General Content', revision: 0 },
      ]);
      expect(blockEntry?.topic).toEqual([
        { topic_id: 'block-1', topic_name: 'Weapons', revision: 0 },
      ]);
    });

    it('groups multiple topics under the same action', async () => {
      mockProfileList.mockResolvedValue({
        ai_profiles: [
          { profile_id: 'p-1', profile_name: 'test-profile', active: true, policy: {} },
        ],
      });
      mockProfileUpdate.mockResolvedValue({});

      await service.assignTopicsToProfile('test-profile', [
        { topicId: 'block-1', topicName: 'Weapons', action: 'block' },
        { topicId: 'block-2', topicName: 'Drugs', action: 'block' },
      ]);

      const tg = getTopicGuardrails(mockProfileUpdate.mock.calls[0][1]);
      expect(tg['topic-list']).toHaveLength(1);
      expect(tg['topic-list'][0].action).toBe('block');
      expect(tg['topic-list'][0].topic).toHaveLength(2);
    });

    it('replaces stale topics from previous runs', async () => {
      mockProfileList.mockResolvedValue({
        ai_profiles: [
          {
            profile_id: 'p-1',
            profile_name: 'test-profile',
            active: true,
            policy: {
              'ai-security-profiles': [
                {
                  'model-type': 'default',
                  'model-configuration': {
                    'model-protection': [
                      {
                        name: 'topic-guardrails',
                        action: 'block',
                        options: [],
                        'topic-list': [
                          {
                            action: 'block',
                            topic: [{ topic_id: 'stale-1', topic_name: 'Old' }],
                          },
                        ],
                      },
                    ],
                  },
                },
              ],
            },
          },
        ],
      });
      mockProfileUpdate.mockResolvedValue({});

      await service.assignTopicsToProfile('test-profile', [
        { topicId: 'allow-1', topicName: 'General', action: 'allow' },
        { topicId: 'block-1', topicName: 'Weapons', action: 'block' },
      ]);

      const tg = getTopicGuardrails(mockProfileUpdate.mock.calls[0][1]);
      expect(tg['topic-list']).toHaveLength(2);
      const blockEntry = tg['topic-list'].find((tl) => tl.action === 'block');
      expect(blockEntry?.topic).toEqual([
        { topic_id: 'block-1', topic_name: 'Weapons', revision: 0 },
      ]);
    });

    it('defaults guardrail-level action to block', async () => {
      mockProfileList.mockResolvedValue({
        ai_profiles: [
          { profile_id: 'p-1', profile_name: 'test-profile', active: true, policy: {} },
        ],
      });
      mockProfileUpdate.mockResolvedValue({});

      await service.assignTopicsToProfile('test-profile', [
        { topicId: 'allow-1', topicName: 'General', action: 'allow' },
      ]);

      const tg = getTopicGuardrails(mockProfileUpdate.mock.calls[0][1]);
      expect((tg as Record<string, unknown>).action).toBe('block');
    });

    it('sets guardrail-level action to allow when specified', async () => {
      mockProfileList.mockResolvedValue({
        ai_profiles: [
          { profile_id: 'p-1', profile_name: 'test-profile', active: true, policy: {} },
        ],
      });
      mockProfileUpdate.mockResolvedValue({});

      await service.assignTopicsToProfile(
        'test-profile',
        [{ topicId: 'block-1', topicName: 'Weapons', action: 'block' }],
        'allow',
      );

      const tg = getTopicGuardrails(mockProfileUpdate.mock.calls[0][1]);
      expect((tg as Record<string, unknown>).action).toBe('allow');
    });
  });

  describe('getProfileTopics', () => {
    it('throws when profile not found', async () => {
      mockProfileList.mockResolvedValue({ ai_profiles: [] });
      await expect(service.getProfileTopics('missing')).rejects.toThrow(
        'Profile "missing" not found',
      );
    });

    it('returns empty array for profile with no topic-guardrails', async () => {
      mockProfileList.mockResolvedValue({
        ai_profiles: [
          { profile_id: 'p-1', profile_name: 'test-profile', active: true, policy: {} },
        ],
      });
      const topics = await service.getProfileTopics('test-profile');
      expect(topics).toEqual([]);
    });

    it('returns empty array for profile with empty topic-list', async () => {
      mockProfileList.mockResolvedValue({
        ai_profiles: [
          {
            profile_id: 'p-1',
            profile_name: 'test-profile',
            active: true,
            policy: {
              'ai-security-profiles': [
                {
                  'model-configuration': {
                    'model-protection': [{ name: 'topic-guardrails', 'topic-list': [] }],
                  },
                },
              ],
            },
          },
        ],
      });
      const topics = await service.getProfileTopics('test-profile');
      expect(topics).toEqual([]);
    });

    it('returns topics with full details from listTopics cross-reference', async () => {
      mockProfileList.mockResolvedValue({
        ai_profiles: [
          {
            profile_id: 'p-1',
            profile_name: 'test-profile',
            active: true,
            policy: {
              'ai-security-profiles': [
                {
                  'model-configuration': {
                    'model-protection': [
                      {
                        name: 'topic-guardrails',
                        'topic-list': [
                          {
                            action: 'block',
                            topic: [{ topic_id: 't1', topic_name: 'Weapons' }],
                          },
                          {
                            action: 'allow',
                            topic: [{ topic_id: 't2', topic_name: 'Education' }],
                          },
                        ],
                      },
                    ],
                  },
                },
              ],
            },
          },
        ],
      });
      mockList.mockResolvedValue({
        custom_topics: [
          {
            topic_id: 't1',
            topic_name: 'Weapons',
            description: 'Block weapons',
            examples: ['gun talk'],
          },
          {
            topic_id: 't2',
            topic_name: 'Education',
            description: 'Allow education',
            examples: ['teach me'],
          },
        ],
      });

      const topics = await service.getProfileTopics('test-profile');
      expect(topics).toHaveLength(2);
      expect(topics[0]).toEqual({
        topicId: 't1',
        topicName: 'Weapons',
        action: 'block',
        description: 'Block weapons',
        examples: ['gun talk'],
      });
      expect(topics[1]).toEqual({
        topicId: 't2',
        topicName: 'Education',
        action: 'allow',
        description: 'Allow education',
        examples: ['teach me'],
      });
    });

    it('handles topic in profile but missing from topic list', async () => {
      mockProfileList.mockResolvedValue({
        ai_profiles: [
          {
            profile_id: 'p-1',
            profile_name: 'test-profile',
            active: true,
            policy: {
              'ai-security-profiles': [
                {
                  'model-configuration': {
                    'model-protection': [
                      {
                        name: 'topic-guardrails',
                        'topic-list': [
                          {
                            action: 'block',
                            topic: [{ topic_id: 't-gone', topic_name: 'Deleted' }],
                          },
                        ],
                      },
                    ],
                  },
                },
              ],
            },
          },
        ],
      });
      mockList.mockResolvedValue({ custom_topics: [] });

      const topics = await service.getProfileTopics('test-profile');
      expect(topics).toHaveLength(1);
      expect(topics[0].description).toBe('');
      expect(topics[0].examples).toEqual([]);
    });
  });

  describe('listProfiles', () => {
    it('lists profiles and returns normalized results', async () => {
      mockProfileList.mockResolvedValue({
        ai_profiles: [
          {
            profile_id: 'p-1',
            profile_name: 'Profile One',
            revision: 3,
            active: true,
            created_by: 'admin@example.com',
            last_modified_ts: '2026-01-01T00:00:00Z',
          },
          {
            profile_id: 'p-2',
            profile_name: 'Profile Two',
            revision: 1,
            active: false,
          },
        ],
        next_offset: 2,
      });

      const result = await service.listProfiles();
      expect(result.profiles).toHaveLength(2);
      expect(result.profiles[0].profileId).toBe('p-1');
      expect(result.profiles[0].profileName).toBe('Profile One');
      expect(result.profiles[0].revision).toBe(3);
      expect(result.profiles[0].active).toBe(true);
      expect(result.profiles[1].profileId).toBe('p-2');
      expect(result.nextOffset).toBe(2);
    });

    it('passes pagination options', async () => {
      mockProfileList.mockResolvedValue({ ai_profiles: [] });

      await service.listProfiles({ limit: 10, offset: 5 });
      expect(mockProfileList).toHaveBeenCalledWith({ limit: 10, offset: 5 });
    });

    it('returns empty array when no profiles', async () => {
      mockProfileList.mockResolvedValue({ ai_profiles: [] });

      const result = await service.listProfiles();
      expect(result.profiles).toEqual([]);
    });
  });

  describe('createProfile', () => {
    it('creates a profile via SDK and returns normalized result', async () => {
      mockProfileCreate.mockResolvedValue({
        profile_id: 'p-new',
        profile_name: 'New Profile',
        revision: 0,
        active: true,
        policy: {},
      });

      const result = await service.createProfile({
        profile_name: 'New Profile',
        active: true,
        policy: {},
      });

      expect(result.profileId).toBe('p-new');
      expect(result.profileName).toBe('New Profile');
      expect(mockProfileCreate).toHaveBeenCalledWith({
        profile_name: 'New Profile',
        active: true,
        policy: {},
      });
    });
  });

  describe('updateProfile', () => {
    it('updates a profile via SDK and returns normalized result', async () => {
      mockProfileUpdate.mockResolvedValue({
        profile_id: 'p-1',
        profile_name: 'Updated Profile',
        revision: 4,
        active: true,
        policy: {},
      });

      const result = await service.updateProfile('p-1', {
        profile_name: 'Updated Profile',
        active: true,
        policy: {},
      });

      expect(result.profileId).toBe('p-1');
      expect(result.profileName).toBe('Updated Profile');
      expect(result.revision).toBe(4);
      expect(mockProfileUpdate).toHaveBeenCalledWith('p-1', {
        profile_name: 'Updated Profile',
        active: true,
        policy: {},
      });
    });
  });

  describe('deleteProfile', () => {
    it('deletes a profile via SDK', async () => {
      mockProfileDelete.mockResolvedValue({ message: 'deleted' });

      const result = await service.deleteProfile('p-1');
      expect(result.message).toBe('deleted');
      expect(mockProfileDelete).toHaveBeenCalledWith('p-1');
    });
  });

  describe('forceDeleteProfile', () => {
    it('force-deletes a profile via SDK', async () => {
      mockProfileForceDelete.mockResolvedValue({ message: 'force deleted' });

      const result = await service.forceDeleteProfile('p-1', 'admin@example.com');
      expect(result.message).toBe('force deleted');
      expect(mockProfileForceDelete).toHaveBeenCalledWith('p-1', 'admin@example.com');
    });
  });

  // -------------------------------------------------------------------------
  // API Keys
  // -------------------------------------------------------------------------

  describe('listApiKeys', () => {
    it('lists and normalizes API keys', async () => {
      mockApiKeysList.mockResolvedValue({
        api_keys: [
          { id: 'k1', name: 'key-one', created_at: '2026-01-01' },
          { id: 'k2', name: 'key-two' },
        ],
        next_offset: 2,
      });

      const result = await service.listApiKeys({ limit: 10 });
      expect(result.apiKeys).toHaveLength(2);
      expect(result.apiKeys[0].id).toBe('k1');
      expect(result.apiKeys[0].name).toBe('key-one');
      expect(result.nextOffset).toBe(2);
    });
  });

  describe('createApiKey', () => {
    it('creates an API key', async () => {
      mockApiKeysCreate.mockResolvedValue({ id: 'k-new', name: 'new-key' });

      const result = await service.createApiKey({ name: 'new-key' });
      expect(result.id).toBe('k-new');
      expect(mockApiKeysCreate).toHaveBeenCalledWith({ name: 'new-key' });
    });
  });

  describe('regenerateApiKey', () => {
    it('regenerates an API key', async () => {
      mockApiKeysRegenerate.mockResolvedValue({ id: 'k1', name: 'key-one' });

      const result = await service.regenerateApiKey('k1', { rotate: true });
      expect(result.id).toBe('k1');
      expect(mockApiKeysRegenerate).toHaveBeenCalledWith('k1', { rotate: true });
    });
  });

  describe('deleteApiKey', () => {
    it('deletes an API key', async () => {
      mockApiKeysDelete.mockResolvedValue({ message: 'deleted' });

      const result = await service.deleteApiKey('key-one', 'admin@test.com');
      expect(result.message).toBe('deleted');
      expect(mockApiKeysDelete).toHaveBeenCalledWith('key-one', 'admin@test.com');
    });
  });

  // -------------------------------------------------------------------------
  // Customer Apps
  // -------------------------------------------------------------------------

  describe('listCustomerApps', () => {
    it('lists and normalizes customer apps', async () => {
      mockCustomerAppsList.mockResolvedValue({
        customer_apps: [
          { customer_app_id: 'a1', app_name: 'App One' },
          { customer_app_id: 'a2', app_name: 'App Two' },
        ],
      });

      const result = await service.listCustomerApps();
      expect(result.apps).toHaveLength(2);
      expect(result.apps[0].name).toBe('App One');
    });
  });

  describe('getCustomerApp', () => {
    it('gets a customer app by name', async () => {
      mockCustomerAppsGet.mockResolvedValue({ app_name: 'App One', customer_app_id: 'a1' });

      const result = await service.getCustomerApp('App One');
      expect(result.name).toBe('App One');
      expect(mockCustomerAppsGet).toHaveBeenCalledWith('App One');
    });
  });

  describe('updateCustomerApp', () => {
    it('updates a customer app', async () => {
      mockCustomerAppsUpdate.mockResolvedValue({
        customer_app_id: 'a1',
        app_name: 'Updated App',
      });

      const result = await service.updateCustomerApp('a1', { app_name: 'Updated App' });
      expect(result.name).toBe('Updated App');
    });
  });

  describe('deleteCustomerApp', () => {
    it('deletes a customer app', async () => {
      mockCustomerAppsDelete.mockResolvedValue({
        app_name: 'App One',
        customer_app_id: 'a1',
      });

      const result = await service.deleteCustomerApp('App One', 'admin@test.com');
      expect(result.name).toBe('App One');
    });
  });

  // -------------------------------------------------------------------------
  // Deployment Profiles
  // -------------------------------------------------------------------------

  describe('listDeploymentProfiles', () => {
    it('lists deployment profiles', async () => {
      mockDeploymentProfilesList.mockResolvedValue({
        deployment_profiles: [{ profile_name: 'default', profile_id: 'dp-1' }],
      });

      const result = await service.listDeploymentProfiles();
      expect(result).toHaveLength(1);
      expect(result[0].raw.profile_name).toBe('default');
    });

    it('passes unactivated option', async () => {
      mockDeploymentProfilesList.mockResolvedValue({ deployment_profiles: [] });

      await service.listDeploymentProfiles({ unactivated: true });
      expect(mockDeploymentProfilesList).toHaveBeenCalledWith({ unactivated: true });
    });
  });

  // -------------------------------------------------------------------------
  // DLP Profiles
  // -------------------------------------------------------------------------

  describe('listDlpProfiles', () => {
    it('lists DLP profiles', async () => {
      mockDlpProfilesList.mockResolvedValue({
        dlp_profiles: [{ profile_name: 'dlp-default', profile_id: 'dlp-1' }],
      });

      const result = await service.listDlpProfiles();
      expect(result).toHaveLength(1);
      expect(result[0].raw.profile_name).toBe('dlp-default');
    });
  });

  // -------------------------------------------------------------------------
  // Scan Logs
  // -------------------------------------------------------------------------

  describe('queryScanLogs', () => {
    it('queries scan logs with correct params', async () => {
      mockScanLogsQuery.mockResolvedValue({
        scan_result_for_dashboard: {
          scan_result_entries: [{ action: 'block', app_name: 'test-app', profile_name: 'default' }],
        },
        page_token: 'next-page',
      });

      const result = await service.queryScanLogs({
        timeInterval: 24,
        timeUnit: 'hour',
        pageNumber: 1,
        pageSize: 50,
        filter: 'threat',
      });
      expect(result.results).toHaveLength(1);
      expect(result.pageToken).toBe('next-page');
      expect(mockScanLogsQuery).toHaveBeenCalledWith({
        time_interval: 24,
        time_unit: 'hour',
        pageNumber: 1,
        pageSize: 50,
        filter: 'threat',
        page_token: undefined,
      });
    });
  });
});
