import { describe, expect, it, vi } from 'vitest';
import { applyTopicToProfile } from '../../../src/cli/commands/topics-apply.js';
import { createMockManagementService } from '../../helpers/mocks.js';

describe('topics-apply', () => {
  describe('applyTopicToProfile', () => {
    it('assigns a topic to a profile with block intent', async () => {
      const mgmt = createMockManagementService();
      mgmt.listTopics = vi
        .fn()
        .mockResolvedValue([{ topic_id: 'topic-1', topic_name: 'My Topic', revision: 1 }]);
      mgmt.getProfileTopics = vi.fn().mockResolvedValue([]);
      mgmt.assignTopicsToProfile = vi.fn().mockResolvedValue(undefined);

      const result = await applyTopicToProfile(mgmt, {
        profileName: 'test-profile',
        topicName: 'My Topic',
        intent: 'block',
      });

      expect(result.topicName).toBe('My Topic');
      expect(result.intent).toBe('block');
      expect(mgmt.assignTopicsToProfile).toHaveBeenCalledWith(
        'test-profile',
        [{ topicId: 'topic-1', topicName: 'My Topic', action: 'block' }],
        'allow',
      );
    });

    it('assigns a topic with allow intent', async () => {
      const mgmt = createMockManagementService();
      mgmt.listTopics = vi
        .fn()
        .mockResolvedValue([{ topic_id: 'topic-1', topic_name: 'My Topic', revision: 1 }]);
      mgmt.getProfileTopics = vi.fn().mockResolvedValue([]);
      mgmt.assignTopicsToProfile = vi.fn().mockResolvedValue(undefined);

      await applyTopicToProfile(mgmt, {
        profileName: 'test-profile',
        topicName: 'My Topic',
        intent: 'allow',
      });

      expect(mgmt.assignTopicsToProfile).toHaveBeenCalledWith(
        'test-profile',
        [{ topicId: 'topic-1', topicName: 'My Topic', action: 'allow' }],
        'block',
      );
    });

    it('preserves existing topics when adding a new one (additive)', async () => {
      const mgmt = createMockManagementService();
      mgmt.listTopics = vi.fn().mockResolvedValue([
        { topic_id: 'topic-1', topic_name: 'Existing Topic', revision: 1 },
        { topic_id: 'topic-2', topic_name: 'New Topic', revision: 1 },
      ]);
      mgmt.getProfileTopics = vi
        .fn()
        .mockResolvedValue([{ topicId: 'topic-1', topicName: 'Existing Topic', action: 'block' }]);
      mgmt.assignTopicsToProfile = vi.fn().mockResolvedValue(undefined);

      await applyTopicToProfile(mgmt, {
        profileName: 'test-profile',
        topicName: 'New Topic',
        intent: 'block',
      });

      const call = (mgmt.assignTopicsToProfile as ReturnType<typeof vi.fn>).mock.calls[0];
      const topics = call[1] as Array<{ topicName: string }>;
      expect(topics).toHaveLength(2);
      expect(topics.map((t) => t.topicName)).toContain('Existing Topic');
      expect(topics.map((t) => t.topicName)).toContain('New Topic');
    });

    it('replaces existing topic entry when re-applying same name', async () => {
      const mgmt = createMockManagementService();
      mgmt.listTopics = vi
        .fn()
        .mockResolvedValue([{ topic_id: 'topic-1', topic_name: 'My Topic', revision: 2 }]);
      mgmt.getProfileTopics = vi
        .fn()
        .mockResolvedValue([{ topicId: 'topic-1', topicName: 'My Topic', action: 'block' }]);
      mgmt.assignTopicsToProfile = vi.fn().mockResolvedValue(undefined);

      await applyTopicToProfile(mgmt, {
        profileName: 'test-profile',
        topicName: 'My Topic',
        intent: 'block',
      });

      const call = (mgmt.assignTopicsToProfile as ReturnType<typeof vi.fn>).mock.calls[0];
      const topics = call[1] as Array<{ topicName: string }>;
      expect(topics).toHaveLength(1);
    });

    it('throws when topic not found', async () => {
      const mgmt = createMockManagementService();
      mgmt.listTopics = vi.fn().mockResolvedValue([]);

      await expect(
        applyTopicToProfile(mgmt, {
          profileName: 'test-profile',
          topicName: 'Missing',
          intent: 'block',
        }),
      ).rejects.toThrow(/not found/);
    });
  });
});
