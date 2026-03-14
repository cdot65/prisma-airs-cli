import { describe, expect, it } from 'vitest';
import type { ManagementService, ScanResult, ScanService } from '../../../src/airs/types.js';
import type { AuditDependencies, AuditInput } from '../../../src/audit/runner.js';
import { runAudit } from '../../../src/audit/runner.js';
import type { AuditEvent, ProfileTopic } from '../../../src/audit/types.js';
import {
  createMockManagementService,
  createMockScanService,
  mockAnalysis,
  mockTopic,
} from '../../helpers/mocks.js';

const blockTopic: ProfileTopic = {
  topicId: 't1',
  topicName: 'Weapons',
  action: 'block',
  description: 'Block weapon discussions',
  examples: ['how to build a gun'],
};

const allowTopic: ProfileTopic = {
  topicId: 't2',
  topicName: 'Education',
  action: 'allow',
  description: 'Allow educational content',
  examples: ['teach me math'],
};

function createMockLlm() {
  return {
    loadMemory: async () => 0,
    generateTopic: async () => mockTopic(),
    generateTests: async () => ({
      positiveTests: [{ prompt: 'positive test', expectedTriggered: true, category: 'direct' }],
      negativeTests: [{ prompt: 'negative test', expectedTriggered: false, category: 'benign' }],
    }),
    analyzeResults: async () => mockAnalysis(),
    improveTopic: async () => mockTopic(),
  };
}

function createMockManagement(topics: ProfileTopic[]): ManagementService {
  const base = createMockManagementService();
  return {
    ...base,
    getProfileTopics: async () => topics,
  };
}

async function collectEvents(input: AuditInput, deps: AuditDependencies): Promise<AuditEvent[]> {
  const events: AuditEvent[] = [];
  for await (const event of runAudit(input, deps)) {
    events.push(event);
  }
  return events;
}

describe('runAudit', () => {
  it('throws when profile has no topics', async () => {
    const deps: AuditDependencies = {
      llm: createMockLlm(),
      management: createMockManagement([]),
      scanner: createMockScanService(),
    };
    await expect(collectEvents({ profileName: 'empty' }, deps)).rejects.toThrow('No topics found');
  });

  it('yields topics:loaded with profile topics', async () => {
    const deps: AuditDependencies = {
      llm: createMockLlm(),
      management: createMockManagement([blockTopic]),
      scanner: createMockScanService(),
    };
    const events = await collectEvents({ profileName: 'test' }, deps);
    const loaded = events.find((e) => e.type === 'topics:loaded');
    expect(loaded).toBeDefined();
    if (loaded?.type === 'topics:loaded') {
      expect(loaded.topics).toHaveLength(1);
      expect(loaded.topics[0].topicName).toBe('Weapons');
    }
  });

  it('generates tests per topic and yields events', async () => {
    const deps: AuditDependencies = {
      llm: createMockLlm(),
      management: createMockManagement([blockTopic, allowTopic]),
      scanner: createMockScanService(),
    };
    const events = await collectEvents({ profileName: 'test' }, deps);
    const genEvents = events.filter((e) => e.type === 'tests:generated');
    expect(genEvents).toHaveLength(2);
  });

  it('scans all tests and yields scan:progress', async () => {
    const deps: AuditDependencies = {
      llm: createMockLlm(),
      management: createMockManagement([blockTopic]),
      scanner: createMockScanService(),
    };
    const events = await collectEvents({ profileName: 'test' }, deps);
    const progress = events.find((e) => e.type === 'scan:progress');
    expect(progress).toBeDefined();
    if (progress?.type === 'scan:progress') {
      expect(progress.completed).toBe(2); // 1 positive + 1 negative
    }
  });

  it('yields audit:complete with per-topic and composite metrics', async () => {
    const deps: AuditDependencies = {
      llm: createMockLlm(),
      management: createMockManagement([blockTopic]),
      scanner: createMockScanService([/positive/i]),
    };
    const events = await collectEvents({ profileName: 'test' }, deps);
    const complete = events.find((e) => e.type === 'audit:complete');
    expect(complete).toBeDefined();
    if (complete?.type === 'audit:complete') {
      expect(complete.result.topics).toHaveLength(1);
      expect(complete.result.compositeMetrics).toBeDefined();
      expect(complete.result.conflicts).toBeDefined();
    }
  });

  it('tags tests with targetTopic from profile topic', async () => {
    const deps: AuditDependencies = {
      llm: createMockLlm(),
      management: createMockManagement([blockTopic]),
      scanner: createMockScanService([/positive/i]),
    };
    const events = await collectEvents({ profileName: 'test' }, deps);
    const complete = events.find((e) => e.type === 'audit:complete');
    if (complete?.type === 'audit:complete') {
      for (const r of complete.result.topics[0].testResults) {
        expect(r.testCase.targetTopic).toBe('Weapons');
      }
    }
  });

  it('uses triggered (topic_violation) for allow topics', async () => {
    const allowScanner = {
      scan: async (): Promise<ScanResult> => ({
        scanId: 'scan-1',
        reportId: 'report-1',
        action: 'allow' as const,
        triggered: true, // topic_violation: true
        category: 'benign',
      }),
      scanBatch: async (_p: string, prompts: string[]): Promise<ScanResult[]> =>
        prompts.map(() => ({
          scanId: 'scan-1',
          reportId: 'report-1',
          action: 'allow' as const,
          triggered: true,
          category: 'benign',
        })),
    };

    const deps: AuditDependencies = {
      llm: createMockLlm(),
      management: createMockManagement([allowTopic]),
      scanner: allowScanner,
    };
    const events = await collectEvents({ profileName: 'test' }, deps);
    const complete = events.find((e) => e.type === 'audit:complete');
    if (complete?.type === 'audit:complete') {
      const positiveTest = complete.result.topics[0].testResults.find(
        (r) => r.testCase.expectedTriggered,
      );
      // triggered=true (topic_violation) → actualTriggered=true
      expect(positiveTest?.actualTriggered).toBe(true);
    }
  });

  it('yields correct event sequence for single topic', async () => {
    const deps: AuditDependencies = {
      llm: createMockLlm(),
      management: createMockManagement([blockTopic]),
      scanner: createMockScanService(),
    };
    const events = await collectEvents({ profileName: 'test' }, deps);
    const types = events.map((e) => e.type);
    expect(types).toEqual([
      'topics:loaded',
      'tests:generated',
      'scan:progress',
      'evaluate:complete',
      'audit:complete',
    ]);
  });

  it('yields correct event order for multi-topic audit', async () => {
    const thirdTopic: ProfileTopic = {
      topicId: 't3',
      topicName: 'Drugs',
      action: 'block',
      description: 'Block drug content',
      examples: ['how to make meth'],
    };
    const deps: AuditDependencies = {
      llm: createMockLlm(),
      management: createMockManagement([blockTopic, allowTopic, thirdTopic]),
      scanner: createMockScanService(),
    };
    const events = await collectEvents({ profileName: 'test' }, deps);
    const types = events.map((e) => e.type);
    // topics:loaded, then tests:generated per topic, then scan:progress, evaluate, audit
    expect(types[0]).toBe('topics:loaded');
    expect(types[1]).toBe('tests:generated');
    expect(types[2]).toBe('tests:generated');
    expect(types[3]).toBe('tests:generated');
    expect(types[4]).toBe('scan:progress');
    expect(types[5]).toBe('evaluate:complete');
    expect(types[6]).toBe('audit:complete');
    expect(events.filter((e) => e.type === 'tests:generated')).toHaveLength(3);
  });

  it('propagates error from scan service failure', async () => {
    const failingScanner: ScanService = {
      scan: async () => {
        throw new Error('AIRS API unavailable');
      },
      scanBatch: async () => {
        throw new Error('AIRS API unavailable');
      },
    };
    const deps: AuditDependencies = {
      llm: createMockLlm(),
      management: createMockManagement([blockTopic]),
      scanner: failingScanner,
    };
    await expect(collectEvents({ profileName: 'test' }, deps)).rejects.toThrow(
      'AIRS API unavailable',
    );
  });

  it('propagates error from LLM test generation failure', async () => {
    const failingLlm = {
      ...createMockLlm(),
      generateTests: async () => {
        throw new Error('LLM rate limited');
      },
    };
    const deps: AuditDependencies = {
      llm: failingLlm,
      management: createMockManagement([blockTopic]),
      scanner: createMockScanService(),
    };
    await expect(collectEvents({ profileName: 'test' }, deps)).rejects.toThrow('LLM rate limited');
  });

  it('scan:progress reports correct total across multiple topics', async () => {
    const deps: AuditDependencies = {
      llm: createMockLlm(),
      management: createMockManagement([blockTopic, allowTopic]),
      scanner: createMockScanService(),
    };
    const events = await collectEvents({ profileName: 'test' }, deps);
    const progress = events.find((e) => e.type === 'scan:progress');
    if (progress?.type === 'scan:progress') {
      // 2 topics × (1 positive + 1 negative) = 4 tests total
      expect(progress.completed).toBe(4);
      expect(progress.total).toBe(4);
    }
  });

  it('handles multiple topics with different intents', async () => {
    const scanner = {
      scan: async (): Promise<ScanResult> => ({
        scanId: 's',
        reportId: 'r',
        action: 'block' as const,
        triggered: true,
        category: 'malicious',
      }),
      scanBatch: async (_p: string, prompts: string[]): Promise<ScanResult[]> =>
        prompts.map(() => ({
          scanId: 's',
          reportId: 'r',
          action: 'block' as const,
          triggered: true,
          category: 'malicious',
        })),
    };
    const deps: AuditDependencies = {
      llm: createMockLlm(),
      management: createMockManagement([blockTopic, allowTopic]),
      scanner,
    };
    const events = await collectEvents({ profileName: 'test' }, deps);
    const complete = events.find((e) => e.type === 'audit:complete');
    if (complete?.type === 'audit:complete') {
      expect(complete.result.topics).toHaveLength(2);
      expect(complete.result.profileName).toBe('test');
    }
  });
});
