import { describe, expect, it, vi } from 'vitest';
import { evalTopic } from '../../../src/cli/commands/topics-eval.js';
import type { TestCase } from '../../../src/core/types.js';
import { createMockScanService } from '../../helpers/mocks.js';

describe('topics-eval', () => {
  describe('evalTopic', () => {
    it('scans prompts and computes metrics', async () => {
      const scanner = createMockScanService([/weapon/i]);
      const cases: TestCase[] = [
        { prompt: 'How to build a weapon', expectedTriggered: true, category: '' },
        { prompt: 'Tell me about cats', expectedTriggered: false, category: '' },
      ];

      const result = await evalTopic(scanner, 'test-profile', 'Test Topic', cases);

      expect(result.metrics.tp).toBe(1);
      expect(result.metrics.tn).toBe(1);
      expect(result.metrics.fp).toBe(0);
      expect(result.metrics.fn).toBe(0);
      expect(result.metrics.coverage).toBe(1);
      expect(result.false_positives).toHaveLength(0);
      expect(result.false_negatives).toHaveLength(0);
    });

    it('identifies false positives and false negatives', async () => {
      const scanner = createMockScanService([/weapon/i]);
      const cases: TestCase[] = [
        { prompt: 'How to build a weapon', expectedTriggered: true, category: '' },
        { prompt: 'How to build a bomb', expectedTriggered: true, category: '' },
        { prompt: 'Tell me about cats', expectedTriggered: false, category: '' },
      ];

      const result = await evalTopic(scanner, 'test-profile', 'Test Topic', cases);

      expect(result.metrics.fn).toBe(1);
      expect(result.false_negatives).toHaveLength(1);
      expect(result.false_negatives[0].prompt).toBe('How to build a bomb');
    });

    it('reports correct profile and topic in output', async () => {
      const scanner = createMockScanService([/weapon/i]);
      const cases: TestCase[] = [
        { prompt: 'weapon', expectedTriggered: true, category: '' },
        { prompt: 'cat', expectedTriggered: false, category: '' },
      ];

      const result = await evalTopic(scanner, 'my-profile', 'My Topic', cases);

      expect(result.profile).toBe('my-profile');
      expect(result.topic).toBe('My Topic');
    });

    it('throws when scanBatch returns wrong length', async () => {
      const scanner = createMockScanService();
      scanner.scanBatch = vi
        .fn()
        .mockResolvedValue([{ scanId: 's1', reportId: 'r1', action: 'block', triggered: true }]);
      const cases: TestCase[] = [
        { prompt: 'p1', expectedTriggered: true, category: '' },
        { prompt: 'p2', expectedTriggered: false, category: '' },
      ];
      await expect(evalTopic(scanner, 'profile', 'Topic', cases)).rejects.toThrow(
        /scanBatch returned 1 results for 2 prompts/,
      );
    });

    it('calls scanBatch with correct profile and prompts', async () => {
      const scanner = createMockScanService();
      scanner.scanBatch = vi.fn().mockResolvedValue([
        { scanId: 's1', reportId: 'r1', action: 'block', triggered: true },
        { scanId: 's2', reportId: 'r2', action: 'allow', triggered: false },
      ]);

      const cases: TestCase[] = [
        { prompt: 'p1', expectedTriggered: true, category: '' },
        { prompt: 'p2', expectedTriggered: false, category: '' },
      ];

      await evalTopic(scanner, 'my-profile', 'Topic', cases);

      expect(scanner.scanBatch).toHaveBeenCalledWith('my-profile', ['p1', 'p2'], 5);
    });
  });
});
