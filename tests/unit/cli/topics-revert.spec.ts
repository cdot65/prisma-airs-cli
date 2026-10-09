import { describe, expect, it, vi } from 'vitest';
import { revertTopic } from '../../../src/cli/commands/topics-revert.js';
import { createMockManagementService } from '../../helpers/mocks.js';

describe('topics-revert', () => {
  describe('revertTopic', () => {
    it('removes topic from profile and deletes it', async () => {
      const mgmt = createMockManagementService();
      mgmt.forceDeleteTopic = vi.fn();
      mgmt.listTopics = vi
        .fn()
        .mockResolvedValue([{ topic_id: 'topic-1', topic_name: 'My Topic' }]);
      mgmt.getProfileTopics = vi
        .fn()
        .mockResolvedValue([{ topicId: 'topic-1', topicName: 'My Topic', action: 'block' }]);
      mgmt.assignTopicsToProfile = vi.fn().mockResolvedValue(undefined);
      mgmt.deleteTopic = vi.fn().mockResolvedValue(undefined);

      const result = await revertTopic(mgmt, 'test-profile', 'My Topic');

      expect(result.deleted).toEqual(['topic-1']);
      expect(mgmt.deleteTopic).toHaveBeenCalledWith('topic-1');
      expect(mgmt.forceDeleteTopic).not.toHaveBeenCalled();
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
      mgmt.deleteTopic = vi.fn().mockResolvedValue(undefined);

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
    it('refuses deleting a topic absent from the selected profile protections', async () => {
      const mgmt = createMockManagementService();
      mgmt.assignTopicsToProfile = vi.fn();
      mgmt.deleteTopic = vi.fn();
      mgmt.forceDeleteTopic = vi.fn();
      mgmt.listTopics = vi
        .fn()
        .mockResolvedValue([{ topic_id: 'topic-1', topic_name: 'Unassigned' }]);
      mgmt.getProfileTopics = vi.fn().mockResolvedValue([]);
      await expect(revertTopic(mgmt, 'test-profile', 'Unassigned')).rejects.toThrow(/not assigned/);
      expect(mgmt.assignTopicsToProfile).not.toHaveBeenCalled();
      expect(mgmt.deleteTopic).not.toHaveBeenCalled();
      expect(mgmt.forceDeleteTopic).not.toHaveBeenCalled();
    });
    it('checks retained legacy references before any selected-direction mutation', async () => {
      const mgmt = createMockManagementService();
      mgmt.listTopics = vi
        .fn()
        .mockResolvedValue([{ topic_id: 'topic-1', topic_name: 'Restricted' }]);
      mgmt.getProfileTopics = vi
        .fn()
        .mockResolvedValueOnce([
          {
            topicId: 'topic-1',
            topicName: 'Restricted',
            action: 'block',
            direction: 'response',
            aiProfileIndex: 0,
          },
        ])
        .mockResolvedValueOnce([{ topicId: 'topic-1', topicName: 'Restricted', action: 'block' }]);
      mgmt.assignTopicsToProfile = vi.fn();
      mgmt.deleteTopic = vi.fn();
      await expect(
        revertTopic(mgmt, 'Directional', 'Restricted', { direction: 'response' }),
      ).rejects.toThrow(/referenced in another/);
      expect(mgmt.getProfileTopics).toHaveBeenLastCalledWith('Directional', undefined, {
        includeInactive: true,
      });
      expect(mgmt.assignTopicsToProfile).not.toHaveBeenCalled();
      expect(mgmt.deleteTopic).not.toHaveBeenCalled();
    });
    it('passes selectors through detachment and uses ordinary deletion for a selected direction', async () => {
      const mgmt = createMockManagementService();
      mgmt.listTopics = vi
        .fn()
        .mockResolvedValue([{ topic_id: 'topic-1', topic_name: 'Restricted' }]);
      mgmt.getProfileTopics = vi.fn().mockResolvedValue([
        {
          topicId: 'topic-1',
          topicName: 'Restricted',
          action: 'block',
          direction: 'response',
          aiProfileIndex: 1,
        },
      ]);
      mgmt.assignTopicsToProfile = vi.fn();
      mgmt.deleteTopic = vi.fn();
      mgmt.forceDeleteTopic = vi.fn();
      const selector = { direction: 'response', aiProfileIndex: 1 } as const;
      await revertTopic(mgmt, 'Directional', 'Restricted', selector);
      expect(mgmt.assignTopicsToProfile).toHaveBeenCalledWith('Directional', [], 'block', selector);
      expect(mgmt.deleteTopic).toHaveBeenCalledWith('topic-1');
      expect(mgmt.forceDeleteTopic).not.toHaveBeenCalled();
    });
  });
});
