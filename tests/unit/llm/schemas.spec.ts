import { describe, expect, it } from 'vitest';
import {
  AnalysisReportSchema,
  CustomTopicSchema,
  TestSuiteSchema,
} from '../../../src/llm/schemas.js';

describe('LLM schemas', () => {
  describe('CustomTopicSchema', () => {
    it('accepts valid topic', () => {
      const result = CustomTopicSchema.safeParse({
        name: 'Weapons Discussion',
        description: 'Blocks conversations about weapons manufacturing',
        examples: ['How to make a weapon', 'Where to buy ammunition'],
      });
      expect(result.success).toBe(true);
    });

    it('rejects empty name', () => {
      const result = CustomTopicSchema.safeParse({
        name: '',
        description: 'Valid desc',
        examples: ['Valid example'],
      });
      expect(result.success).toBe(false);
    });

    it('accepts name over 100 chars (clamping handled by service)', () => {
      const result = CustomTopicSchema.safeParse({
        name: 'a'.repeat(101),
        description: 'Valid desc',
        examples: ['Valid example', 'Another example'],
      });
      expect(result.success).toBe(true);
    });

    it('accepts description over 250 chars (clamping handled by service)', () => {
      const result = CustomTopicSchema.safeParse({
        name: 'Valid',
        description: 'a'.repeat(251),
        examples: ['Valid example', 'Another example'],
      });
      expect(result.success).toBe(true);
    });

    it('rejects more than 5 examples', () => {
      const result = CustomTopicSchema.safeParse({
        name: 'Valid',
        description: 'Valid desc',
        examples: ['a', 'b', 'c', 'd', 'e', 'f'],
      });
      expect(result.success).toBe(false);
    });

    it('rejects fewer than 2 examples', () => {
      expect(
        CustomTopicSchema.safeParse({ name: 'V', description: 'V', examples: [] }).success,
      ).toBe(false);
      expect(
        CustomTopicSchema.safeParse({ name: 'V', description: 'V', examples: ['one'] }).success,
      ).toBe(false);
    });

    it('accepts exactly 2 examples', () => {
      const result = CustomTopicSchema.safeParse({
        name: 'Valid',
        description: 'Valid desc',
        examples: ['one', 'two'],
      });
      expect(result.success).toBe(true);
    });

    it('accepts example over 250 chars (clamping handled by service)', () => {
      const result = CustomTopicSchema.safeParse({
        name: 'Valid',
        description: 'Valid desc',
        examples: ['a'.repeat(251), 'Another example'],
      });
      expect(result.success).toBe(true);
    });
  });

  describe('TestSuiteSchema', () => {
    it('accepts valid test suite', () => {
      const result = TestSuiteSchema.safeParse({
        positiveTests: [{ prompt: 'bad prompt', expectedTriggered: true, category: 'direct' }],
        negativeTests: [{ prompt: 'good prompt', expectedTriggered: false, category: 'benign' }],
      });
      expect(result.success).toBe(true);
    });

    it('rejects empty positive tests', () => {
      const result = TestSuiteSchema.safeParse({
        positiveTests: [],
        negativeTests: [{ prompt: 'good', expectedTriggered: false, category: 'benign' }],
      });
      expect(result.success).toBe(false);
    });

    it('rejects missing fields on test case', () => {
      const result = TestSuiteSchema.safeParse({
        positiveTests: [{ prompt: 'test' }],
        negativeTests: [{ prompt: 'test', expectedTriggered: false, category: 'x' }],
      });
      expect(result.success).toBe(false);
    });
  });

  describe('AnalysisReportSchema', () => {
    it('accepts valid analysis report', () => {
      const result = AnalysisReportSchema.safeParse({
        summary: 'The guardrail is too broad',
        falsePositivePatterns: ['Catches benign cooking terms'],
        falseNegativePatterns: ['Misses coded language'],
        suggestions: ['Narrow the description to focus on manufacturing'],
      });
      expect(result.success).toBe(true);
    });

    it('accepts empty pattern arrays', () => {
      const result = AnalysisReportSchema.safeParse({
        summary: 'Perfect results',
        falsePositivePatterns: [],
        falseNegativePatterns: [],
        suggestions: ['No changes needed'],
      });
      expect(result.success).toBe(true);
    });

    it('rejects empty suggestions', () => {
      const result = AnalysisReportSchema.safeParse({
        summary: 'Summary',
        falsePositivePatterns: [],
        falseNegativePatterns: [],
        suggestions: [],
      });
      expect(result.success).toBe(false);
    });
  });
});
