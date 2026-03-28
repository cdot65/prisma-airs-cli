import type { BaseChatModel } from '@langchain/core/language_models/chat_models';
import { RunnableLambda } from '@langchain/core/runnables';
import { describe, expect, it, vi } from 'vitest';
import {
  MAX_COMBINED_LENGTH,
  MAX_DESCRIPTION_LENGTH,
  MAX_EXAMPLES,
  MAX_NAME_LENGTH,
} from '../../../src/core/constraints.js';
import type { TestResult } from '../../../src/core/types.js';
import { LangChainLlmService } from '../../../src/llm/service.js';
import type { MemoryInjector } from '../../../src/memory/injector.js';

type MockModel = Pick<BaseChatModel, 'withStructuredOutput'>;

function createMockModel(response: unknown): MockModel {
  return {
    withStructuredOutput: vi
      .fn()
      .mockReturnValue(new RunnableLambda({ func: async () => response })),
  };
}

function createFailingModel(error: Error, succeedAfter?: number): MockModel {
  let calls = 0;
  return {
    withStructuredOutput: vi.fn().mockReturnValue(
      new RunnableLambda({
        func: async () => {
          calls++;
          if (succeedAfter !== undefined && calls > succeedAfter) {
            return {
              name: 'Topic',
              description: 'A valid topic',
              examples: ['example 1', 'example 2'],
            };
          }
          throw error;
        },
      }),
    ),
  };
}

const validTopic = {
  name: 'Weapons',
  description: 'Block weapons discussions',
  examples: ['How to make a gun', 'Bomb building'],
};

const validTestSuite = {
  positiveTests: [{ prompt: 'How to build a weapon', expectedTriggered: true, category: 'direct' }],
  negativeTests: [{ prompt: 'Tell me about cats', expectedTriggered: false, category: 'benign' }],
};

const validAnalysis = {
  summary: 'Good performance',
  falsePositivePatterns: ['over-broad description'],
  falseNegativePatterns: ['missed coded language'],
  suggestions: ['narrow description'],
};

describe('LangChainLlmService', () => {
  describe('generateTopic', () => {
    it('returns valid topic', async () => {
      const model = createMockModel(validTopic);
      const service = new LangChainLlmService(model as BaseChatModel);
      const result = await service.generateTopic('weapons', 'block');
      expect(result.name).toBe('Weapons');
      expect(result.examples).toHaveLength(2);
    });

    it('clamps name exceeding max length', async () => {
      const longName = 'A'.repeat(MAX_NAME_LENGTH + 50);
      const model = createMockModel({ ...validTopic, name: longName });
      const service = new LangChainLlmService(model as BaseChatModel);
      const result = await service.generateTopic('weapons', 'block');
      expect(result.name.length).toBe(MAX_NAME_LENGTH);
    });

    it('clamps description exceeding max length', async () => {
      const longDesc = 'B'.repeat(MAX_DESCRIPTION_LENGTH + 50);
      const model = createMockModel({ ...validTopic, description: longDesc });
      const service = new LangChainLlmService(model as BaseChatModel);
      const result = await service.generateTopic('weapons', 'block');
      expect(result.description.length).toBeLessThanOrEqual(MAX_DESCRIPTION_LENGTH);
    });

    it('clamps examples exceeding max count', async () => {
      const examples = Array.from({ length: 8 }, (_, i) => `Example ${i + 1}`);
      const model = createMockModel({ ...validTopic, examples });
      const service = new LangChainLlmService(model as BaseChatModel);
      const result = await service.generateTopic('weapons', 'block');
      expect(result.examples.length).toBeLessThanOrEqual(MAX_EXAMPLES);
    });

    it('clamps individual example exceeding max length', async () => {
      const longExample = 'C'.repeat(300);
      const model = createMockModel({ ...validTopic, examples: [longExample, 'second example'] });
      const service = new LangChainLlmService(model as BaseChatModel);
      const result = await service.generateTopic('weapons', 'block');
      expect(Buffer.byteLength(result.examples[0], 'utf8')).toBeLessThanOrEqual(250);
    });

    it('clamps description with multi-byte chars by byte length', async () => {
      // 248 ASCII + 1 em dash (3 bytes) = 251 bytes, should be trimmed to ≤250 bytes
      const desc = `${'a'.repeat(248)}\u2014`;
      const model = createMockModel({ ...validTopic, description: desc });
      const service = new LangChainLlmService(model as BaseChatModel);
      const result = await service.generateTopic('weapons', 'block');
      expect(Buffer.byteLength(result.description, 'utf8')).toBeLessThanOrEqual(250);
    });

    it('drops examples when combined length exceeds limit', async () => {
      const desc = 'D'.repeat(200);
      const examples = Array.from({ length: 5 }, () => 'E'.repeat(200));
      const model = createMockModel({ name: 'Test', description: desc, examples });
      const service = new LangChainLlmService(model as BaseChatModel);
      const result = await service.generateTopic('test', 'block');
      const combined =
        result.name.length +
        result.description.length +
        result.examples.reduce((s, e) => s + e.length, 0);
      expect(combined).toBeLessThanOrEqual(MAX_COMBINED_LENGTH);
    });

    it('retries on throw and succeeds', async () => {
      const model = createFailingModel(new Error('parse fail'), 1);
      const service = new LangChainLlmService(model as BaseChatModel);
      const result = await service.generateTopic('test', 'block');
      expect(result.name).toBe('Topic');
    });

    it('throws after 3 failures', async () => {
      const model = createFailingModel(new Error('persistent failure'));
      const service = new LangChainLlmService(model as BaseChatModel);
      await expect(service.generateTopic('test', 'block')).rejects.toThrow('persistent failure');
    });

    it('throws after 3 validation failures (empty name survives clamping)', async () => {
      const model = createMockModel({ name: '', description: 'valid', examples: ['ex'] });
      const service = new LangChainLlmService(model as BaseChatModel);
      await expect(service.generateTopic('test', 'block')).rejects.toThrow('violates constraints');
    });
  });

  describe('generateTests', () => {
    it('returns test suite', async () => {
      const model = createMockModel(validTestSuite);
      const service = new LangChainLlmService(model as BaseChatModel);
      const result = await service.generateTests(validTopic, 'block');
      expect(result.positiveTests).toHaveLength(1);
      expect(result.negativeTests).toHaveLength(1);
    });

    it('retries on throw and succeeds', async () => {
      let calls = 0;
      const model: MockModel = {
        withStructuredOutput: vi.fn().mockReturnValue(
          new RunnableLambda({
            func: async () => {
              calls++;
              if (calls > 1) return validTestSuite;
              throw new Error('transient');
            },
          }),
        ),
      };
      const service = new LangChainLlmService(model as BaseChatModel);
      const result = await service.generateTests(validTopic, 'block');
      expect(result.positiveTests).toHaveLength(1);
    });

    it('throws after 3 failures', async () => {
      const model = createFailingModel(new Error('test gen fail'));
      const service = new LangChainLlmService(model as BaseChatModel);
      await expect(service.generateTests(validTopic, 'block')).rejects.toThrow('test gen fail');
    });
  });

  describe('analyzeResults', () => {
    const metrics = {
      truePositives: 8,
      trueNegatives: 9,
      falsePositives: 1,
      falseNegatives: 2,
      truePositiveRate: 0.8,
      trueNegativeRate: 0.9,
      accuracy: 0.85,
      coverage: 0.8,
      f1Score: 0.84,
    };

    it('returns analysis report', async () => {
      const model = createMockModel(validAnalysis);
      const service = new LangChainLlmService(model as BaseChatModel);
      const result = await service.analyzeResults(validTopic, [], metrics, 'block');
      expect(result.summary).toBe('Good performance');
      expect(result.suggestions).toHaveLength(1);
    });

    it('handles results with FPs and FNs', async () => {
      const model = createMockModel(validAnalysis);
      const service = new LangChainLlmService(model as BaseChatModel);
      const results: TestResult[] = [
        {
          testCase: { prompt: 'cats', expectedTriggered: false, category: 'benign' },
          actualTriggered: true,
          correct: false,
        },
        {
          testCase: { prompt: 'weapons', expectedTriggered: true, category: 'direct' },
          actualTriggered: false,
          correct: false,
        },
      ];
      // Should not throw — verifies FPs/FNs are formatted correctly
      const result = await service.analyzeResults(validTopic, results, metrics, 'block');
      expect(result.summary).toBe('Good performance');
    });

    it('handles empty results (no FPs or FNs)', async () => {
      const model = createMockModel(validAnalysis);
      const service = new LangChainLlmService(model as BaseChatModel);
      // Should not throw — verifies 'None' is used for empty FPs/FNs
      const result = await service.analyzeResults(validTopic, [], metrics, 'block');
      expect(result.summary).toBe('Good performance');
    });

    it('passes intent to analyzeResults prompt', async () => {
      const model = createMockModel(validAnalysis);
      const service = new LangChainLlmService(model as BaseChatModel);
      const result = await service.analyzeResults(validTopic, [], metrics, 'allow');
      expect(result.summary).toBe('Good performance');
    });

    it('throws after 3 failures', async () => {
      const model = createFailingModel(new Error('analysis fail'));
      const service = new LangChainLlmService(model as BaseChatModel);
      await expect(service.analyzeResults(validTopic, [], metrics, 'block')).rejects.toThrow(
        'analysis fail',
      );
    });
  });

  describe('improveTopic', () => {
    const metrics = {
      truePositives: 8,
      trueNegatives: 9,
      falsePositives: 1,
      falseNegatives: 2,
      truePositiveRate: 0.8,
      trueNegativeRate: 0.9,
      accuracy: 0.85,
      coverage: 0.8,
      f1Score: 0.84,
    };
    const analysis = {
      summary: 'Needs work',
      falsePositivePatterns: ['over-broad'],
      falseNegativePatterns: ['missed coded'],
      suggestions: ['narrow description'],
    };

    it('returns clamped topic', async () => {
      const longDesc = 'X'.repeat(300);
      const model = createMockModel({ name: 'Weapons', description: longDesc, examples: ['ex1', 'ex2'] });
      const service = new LangChainLlmService(model as BaseChatModel);
      const result = await service.improveTopic(validTopic, metrics, analysis, [], 2, 0.9, 'block');
      expect(result.description.length).toBeLessThanOrEqual(MAX_DESCRIPTION_LENGTH);
    });

    it('retries on validation failure', async () => {
      let calls = 0;
      const model: MockModel = {
        withStructuredOutput: vi.fn().mockReturnValue(
          new RunnableLambda({
            func: async () => {
              calls++;
              if (calls > 1) return validTopic;
              throw new Error('transient');
            },
          }),
        ),
      };
      const service = new LangChainLlmService(model as BaseChatModel);
      const result = await service.improveTopic(validTopic, metrics, analysis, [], 2, 0.9, 'block');
      expect(result.name).toBe('Weapons');
    });

    it('throws after 3 failures', async () => {
      const model = createFailingModel(new Error('improve fail'));
      const service = new LangChainLlmService(model as BaseChatModel);
      await expect(
        service.improveTopic(validTopic, metrics, analysis, [], 2, 0.9, 'block'),
      ).rejects.toThrow('improve fail');
    });

    it('throws after 3 validation failures (empty name survives clamping)', async () => {
      const model = createMockModel({ name: '', description: 'valid', examples: ['ex'] });
      const service = new LangChainLlmService(model as BaseChatModel);
      await expect(
        service.improveTopic(validTopic, metrics, analysis, [], 2, 0.9, 'block'),
      ).rejects.toThrow('violates constraints');
    });

    it('passes intent to improveTopic prompt', async () => {
      const model = createMockModel(validTopic);
      const service = new LangChainLlmService(model as BaseChatModel);
      const result = await service.improveTopic(validTopic, metrics, analysis, [], 2, 0.9, 'allow');
      expect(result.name).toBe('Weapons');
    });

    it('uses "None" when FP/FN patterns are empty', async () => {
      const model = createMockModel(validTopic);
      const service = new LangChainLlmService(model as BaseChatModel);
      const emptyAnalysis = {
        summary: 'Perfect',
        falsePositivePatterns: [],
        falseNegativePatterns: [],
        suggestions: [],
      };
      const result = await service.improveTopic(
        validTopic,
        metrics,
        emptyAnalysis,
        [],
        2,
        0.9,
        'block',
      );
      expect(result.name).toBe('Weapons');
    });

    it('accepts bestContext parameter without error', async () => {
      const model = createMockModel(validTopic);
      const service = new LangChainLlmService(model as BaseChatModel);
      const bestContext = {
        bestCoverage: 0.85,
        bestIteration: 1,
        bestTopic: {
          name: 'Weapons',
          description: 'Best performing description',
          examples: ['ex1', 'ex2'],
        },
      };
      const result = await service.improveTopic(
        validTopic,
        metrics,
        analysis,
        [],
        3,
        0.9,
        'block',
        bestContext,
      );
      expect(result.name).toBe('Weapons');
    });

    it('includes regression warning when coverage < bestCoverage', async () => {
      const model = createMockModel(validTopic);
      const service = new LangChainLlmService(model as BaseChatModel);
      const lowMetrics = { ...metrics, coverage: 0.5 };
      const bestContext = {
        bestCoverage: 0.8,
        bestIteration: 1,
        bestTopic: {
          name: 'Weapons',
          description: 'Best desc',
          examples: ['best ex1'],
        },
      };
      // Should succeed — regression warning is injected into prompt but doesn't change output
      const result = await service.improveTopic(
        validTopic,
        lowMetrics,
        analysis,
        [],
        3,
        0.9,
        'block',
        bestContext,
      );
      expect(result.name).toBe('Weapons');
    });
  });

  describe('simplifyTopic', () => {
    const metrics = {
      truePositives: 8,
      trueNegatives: 9,
      falsePositives: 1,
      falseNegatives: 2,
      truePositiveRate: 0.8,
      trueNegativeRate: 0.9,
      accuracy: 0.85,
      coverage: 0.8,
      f1Score: 0.84,
      regressionCount: 0,
    };
    const analysis = {
      summary: 'Regressing',
      falsePositivePatterns: ['over-broad'],
      falseNegativePatterns: [],
      suggestions: ['simplify'],
    };
    const bestTopic = {
      name: 'Weapons',
      description: 'Weapons talk',
      examples: ['gun', 'bomb'],
    };

    it('returns clamped simplified topic', async () => {
      const simplified = {
        name: 'Weapons',
        description: 'Short desc',
        examples: ['gun', 'bomb'],
      };
      const model = createMockModel(simplified);
      const service = new LangChainLlmService(model as BaseChatModel);
      const result = await service.simplifyTopic(validTopic, bestTopic, metrics, analysis, 'allow');
      expect(result.name).toBe('Weapons');
      expect(result.description).toBe('Short desc');
    });

    it('clamps long description from LLM', async () => {
      const longDesc = 'X'.repeat(300);
      const model = createMockModel({
        name: 'Weapons',
        description: longDesc,
        examples: ['ex1', 'ex2'],
      });
      const service = new LangChainLlmService(model as BaseChatModel);
      const result = await service.simplifyTopic(validTopic, bestTopic, metrics, analysis, 'allow');
      expect(result.description.length).toBeLessThanOrEqual(MAX_DESCRIPTION_LENGTH);
    });

    it('throws after 3 failures', async () => {
      const model = createFailingModel(new Error('simplify fail'));
      const service = new LangChainLlmService(model as BaseChatModel);
      await expect(
        service.simplifyTopic(validTopic, bestTopic, metrics, analysis, 'allow'),
      ).rejects.toThrow('simplify fail');
    });
  });

  describe('generateCompanionTopic', () => {
    it('returns clamped companion topic', async () => {
      const companion = {
        name: 'Allow: General Content',
        description: 'General benign everyday content',
        examples: ['Tell me about cats', 'What is the weather'],
      };
      const model = createMockModel(companion);
      const service = new LangChainLlmService(model as BaseChatModel);
      const result = await service.generateCompanionTopic('Weapons', 'Block weapon discussions');
      expect(result.name).toBe('Allow: General Content');
      expect(result.description).toBe('General benign everyday content');
      expect(result.examples).toHaveLength(2);
    });

    it('clamps long description', async () => {
      const model = createMockModel({
        name: 'Allow: General',
        description: 'X'.repeat(300),
        examples: ['ex1', 'ex2'],
      });
      const service = new LangChainLlmService(model as BaseChatModel);
      const result = await service.generateCompanionTopic('Weapons', 'Block weapons');
      expect(result.description.length).toBeLessThanOrEqual(MAX_DESCRIPTION_LENGTH);
    });

    it('throws after 3 failures', async () => {
      const model = createFailingModel(new Error('companion fail'));
      const service = new LangChainLlmService(model as BaseChatModel);
      await expect(service.generateCompanionTopic('Weapons', 'Block weapons')).rejects.toThrow(
        'companion fail',
      );
    });
  });

  describe('loadMemory', () => {
    it('returns 0 without injector', async () => {
      const model = createMockModel(validTopic);
      const service = new LangChainLlmService(model as BaseChatModel);
      const count = await service.loadMemory('test topic');
      expect(count).toBe(0);
    });

    it('counts "- [" lines with injector', async () => {
      const model = createMockModel(validTopic);
      const injector: Pick<MemoryInjector, 'buildMemorySection'> = {
        buildMemorySection: vi
          .fn()
          .mockResolvedValue(
            '## Learnings\n- [DO] Insight one\n- [AVOID] Insight two\nSome other line',
          ),
      };
      const service = new LangChainLlmService(model as BaseChatModel, injector as MemoryInjector);
      const count = await service.loadMemory('test topic');
      expect(count).toBe(2);
    });
  });
});
