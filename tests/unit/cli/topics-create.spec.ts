import { describe, expect, it, vi } from 'vitest';
import { createOrUpdateTopic } from '../../../src/cli/commands/topics-create.js';
import { createMockManagementService } from '../../helpers/mocks.js';

describe('topics-create', () => {
  describe('createOrUpdateTopic', () => {
    it('creates a new topic when none exists with that name', async () => {
      const mgmt = createMockManagementService();
      mgmt.listTopics = vi.fn().mockResolvedValue([]);

      const result = await createOrUpdateTopic(mgmt, {
        name: 'Test Topic',
        description: 'A test description',
        examples: ['Example 1', 'Example 2'],
      });

      expect(result.topicId).toBe('topic-1');
      expect(result.topicName).toBe('Test Topic');
      expect(result.created).toBe(true);
    });

    it('updates existing topic when name matches', async () => {
      const mgmt = createMockManagementService();
      mgmt.listTopics = vi
        .fn()
        .mockResolvedValue([
          { topic_id: 'existing-1', topic_name: 'Test Topic', description: 'old', examples: [] },
        ]);
      mgmt.updateTopic = vi.fn().mockResolvedValue({
        topic_id: 'existing-1',
        topic_name: 'Test Topic',
        description: 'A test description',
        examples: ['Example 1', 'Example 2'],
        revision: 2,
      });

      const result = await createOrUpdateTopic(mgmt, {
        name: 'Test Topic',
        description: 'A test description',
        examples: ['Example 1', 'Example 2'],
      });

      expect(result.topicId).toBe('existing-1');
      expect(result.created).toBe(false);
      expect(mgmt.updateTopic).toHaveBeenCalledWith('existing-1', expect.any(Object));
    });

    it('throws on constraint validation failure — name too long', async () => {
      const mgmt = createMockManagementService();
      mgmt.listTopics = vi.fn().mockResolvedValue([]);

      await expect(
        createOrUpdateTopic(mgmt, {
          name: 'x'.repeat(200),
          description: 'A test description',
          examples: ['Example 1', 'Example 2'],
        }),
      ).rejects.toThrow(/bytes/);
    });

    it('throws on constraint validation failure — fewer than 2 examples', async () => {
      const mgmt = createMockManagementService();
      mgmt.listTopics = vi.fn().mockResolvedValue([]);

      await expect(
        createOrUpdateTopic(mgmt, {
          name: 'Test Topic',
          description: 'A test description',
          examples: ['Only one'],
        }),
      ).rejects.toThrow(/at least 2/);
    });

    it('throws on constraint validation failure — more than 5 examples', async () => {
      const mgmt = createMockManagementService();
      mgmt.listTopics = vi.fn().mockResolvedValue([]);

      await expect(
        createOrUpdateTopic(mgmt, {
          name: 'Test Topic',
          description: 'desc',
          examples: ['a', 'b', 'c', 'd', 'e', 'f'],
        }),
      ).rejects.toThrow(/5/);
    });
  });
});
