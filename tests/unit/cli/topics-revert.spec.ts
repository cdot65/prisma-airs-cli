import { describe, expect, it, vi } from 'vitest';
import { revertTopic } from '../../../src/cli/commands/topics-revert.js';
import { createMockManagementService } from '../../helpers/mocks.js';

describe('topics-revert', () => {
  describe('revertTopic', () => {
    it('removes topic from profile and deletes it', async () => {
      const mgmt = createMockManagementService();
      mgmt.listTopics = vi
        .fn()
        .mockResolvedValue([{ topic_id: 'topic-1', topic_name: 'My Topic' }]);
      mgmt.getProfileTopics = vi
        .fn()
        .mockResolvedValue([{ topicId: 'topic-1', topicName: 'My Topic', action: 'block' }]);
      mgmt.assignTopicsToProfile = vi.fn().mockResolvedValue(undefined);
      mgmt.forceDeleteTopic = vi.fn().mockResolvedValue({ message: 'deleted' });

      const result = await revertTopic(mgmt, 'test-profile', 'My Topic');

      expect(result.deleted).toEqual(['topic-1']);
      expect(mgmt.forceDeleteTopic).toHaveBeenCalledWith('topic-1', undefined);
    });

    it('preserves other topics on the profile', async () => {
      const mgmt = createMockManagementService();
      mgmt.listTopics = vi.fn().mockResolvedValue([
        { topic_id: 'topic-1', topic_name: 'Remove Me' },
        { topic_id: 'topic-2', topic_name: 'Keep Me' },
      ]);
      mgmt.getProfileTopics = vi.fn().mockResolvedValue([
        { topicId: 'topic-1', topicName: 'Remove Me', action: 'block' },
        { topicId: 'topic-2', topicName: 'Keep Me', action: 'allow' },
      ]);
      mgmt.assignTopicsToProfile = vi.fn().mockResolvedValue(undefined);
      mgmt.forceDeleteTopic = vi.fn().mockResolvedValue({ message: 'deleted' });

      await revertTopic(mgmt, 'test-profile', 'Remove Me');

      const call = (mgmt.assignTopicsToProfile as ReturnType<typeof vi.fn>).mock.calls[0];
      const topics = call[1] as Array<{ topicName: string }>;
      expect(topics).toHaveLength(1);
      expect(topics[0].topicName).toBe('Keep Me');
    });

    it('throws when topic not found', async () => {
      const mgmt = createMockManagementService();
      mgmt.listTopics = vi.fn().mockResolvedValue([]);

      await expect(revertTopic(mgmt, 'test-profile', 'Missing')).rejects.toThrow(/not found/);
    });
  });
});
