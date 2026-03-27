import { describe, expect, it } from 'vitest';
import {
  buildProfileOverrides,
  buildProfileRequest,
  mergeProfilePolicy,
  type ProfileFlags,
} from '../../../src/cli/builders/profile-builder.js';

describe('buildProfileRequest', () => {
  it('builds minimal request with only name', () => {
    const result = buildProfileRequest({ name: 'Test Profile' });
    expect(result.profile_name).toBe('Test Profile');
    expect(result.active).toBe(true);
    // No policy sections when no protection flags provided
    const aiProfiles = result.policy?.['ai-security-profiles'];
    expect(aiProfiles).toBeUndefined();
  });

  it('respects active=false', () => {
    const result = buildProfileRequest({ name: 'Test', active: false });
    expect(result.active).toBe(false);
  });

  it('builds model-protection from prompt-injection flag', () => {
    const result = buildProfileRequest({
      name: 'Test',
      promptInjection: 'block',
    });
    const mp = getModelProtection(result);
    expect(mp).toContainEqual(
      expect.objectContaining({ name: 'prompt-injection', action: 'block' }),
    );
  });

  it('builds model-protection from toxic-content flag', () => {
    const result = buildProfileRequest({
      name: 'Test',
      toxicContent: 'high:block, moderate:block',
    });
    const mp = getModelProtection(result);
    expect(mp).toContainEqual(
      expect.objectContaining({ name: 'toxic-content', action: 'high:block, moderate:block' }),
    );
  });

  it('builds model-protection from contextual-grounding flag', () => {
    const result = buildProfileRequest({
      name: 'Test',
      contextualGrounding: 'alert',
    });
    const mp = getModelProtection(result);
    expect(mp).toContainEqual(
      expect.objectContaining({ name: 'contextual-grounding', action: 'alert' }),
    );
  });

  it('builds app-protection from malicious-code flag', () => {
    const result = buildProfileRequest({
      name: 'Test',
      maliciousCode: 'block',
    });
    const ap = getAppProtection(result);
    expect(ap?.['malicious-code-protection']).toEqual(
      expect.objectContaining({ name: 'malicious-code-detection', action: 'block' }),
    );
  });

  it('builds app-protection URL categories from comma-separated flags', () => {
    const result = buildProfileRequest({
      name: 'Test',
      urlAction: 'block',
      allowUrlCategories: 'dynamic-dns,grayware',
      blockUrlCategories: 'phishing,malware',
      alertUrlCategories: 'proxy-avoidance',
    });
    const ap = getAppProtection(result);
    expect(ap?.['url-detected-action']).toBe('block');
    expect(ap?.['allow-url-category']?.member).toEqual(['dynamic-dns', 'grayware']);
    expect(ap?.['block-url-category']?.member).toEqual(['phishing', 'malware']);
    expect(ap?.['alert-url-category']?.member).toEqual(['proxy-avoidance']);
  });

  it('builds agent-protection from agent-security flag', () => {
    const result = buildProfileRequest({
      name: 'Test',
      agentSecurity: 'block',
    });
    const config = getModelConfiguration(result);
    const agentProtection = config?.['agent-protection'];
    expect(agentProtection).toContainEqual(
      expect.objectContaining({ name: 'agent-security', action: 'block' }),
    );
  });

  it('builds data-protection with DLP flags', () => {
    const result = buildProfileRequest({
      name: 'Test',
      dlpAction: 'block',
      dlpProfiles: 'IP Addresses,SSN',
      maskDataInline: true,
    });
    const config = getModelConfiguration(result);
    const dp = config?.['data-protection'];
    expect(dp?.['data-leak-detection']?.action).toBe('block');
    expect(dp?.['data-leak-detection']?.['mask-data-inline']).toBe(true);
    expect(dp?.['data-leak-detection']?.member).toEqual([
      { text: 'IP Addresses' },
      { text: 'SSN' },
    ]);
  });

  it('builds data-protection with database-security flags', () => {
    const result = buildProfileRequest({
      name: 'Test',
      dbSecurityCreate: 'block',
      dbSecurityRead: 'alert',
      dbSecurityUpdate: 'block',
      dbSecurityDelete: 'block',
    });
    const config = getModelConfiguration(result);
    const dp = config?.['data-protection'];
    expect(dp?.['database-security']).toContainEqual({
      name: 'database-security-create',
      action: 'block',
    });
    expect(dp?.['database-security']).toContainEqual({
      name: 'database-security-read',
      action: 'alert',
    });
    expect(dp?.['database-security']).toContainEqual({
      name: 'database-security-update',
      action: 'block',
    });
    expect(dp?.['database-security']).toContainEqual({
      name: 'database-security-delete',
      action: 'block',
    });
  });

  it('builds latency config from flags', () => {
    const result = buildProfileRequest({
      name: 'Test',
      inlineTimeoutAction: 'block',
      maxInlineLatency: 1,
    });
    const config = getModelConfiguration(result);
    expect(config?.latency?.['inline-timeout-action']).toBe('block');
    expect(config?.latency?.['max-inline-latency']).toBe(1);
  });

  it('builds mask-data-in-storage flag', () => {
    const result = buildProfileRequest({
      name: 'Test',
      maskDataInStorage: true,
    });
    const config = getModelConfiguration(result);
    expect(config?.['mask-data-in-storage']).toBe(true);
  });

  it('builds full request with all flags', () => {
    const flags: ProfileFlags = {
      name: 'Full Security',
      active: true,
      promptInjection: 'block',
      toxicContent: 'high:block, moderate:block',
      contextualGrounding: 'block',
      maliciousCode: 'block',
      urlAction: 'block',
      allowUrlCategories: 'dynamic-dns',
      blockUrlCategories: 'phishing,malware',
      alertUrlCategories: 'grayware',
      agentSecurity: 'block',
      dlpAction: 'block',
      dlpProfiles: 'IP Addresses',
      maskDataInline: true,
      dbSecurityCreate: 'block',
      dbSecurityRead: 'block',
      dbSecurityUpdate: 'block',
      dbSecurityDelete: 'block',
      inlineTimeoutAction: 'block',
      maxInlineLatency: 1,
      maskDataInStorage: true,
    };
    const result = buildProfileRequest(flags);

    expect(result.profile_name).toBe('Full Security');
    expect(result.active).toBe(true);

    const mp = getModelProtection(result);
    expect(mp).toHaveLength(3); // prompt-injection, toxic-content, contextual-grounding

    const ap = getAppProtection(result);
    expect(ap?.['malicious-code-protection']).toBeDefined();
    expect(ap?.['url-detected-action']).toBe('block');

    const config = getModelConfiguration(result);
    expect(config?.['agent-protection']).toHaveLength(1);
    expect(config?.['data-protection']?.['data-leak-detection']).toBeDefined();
    expect(config?.['data-protection']?.['database-security']).toHaveLength(4);
    expect(config?.latency).toBeDefined();
    expect(config?.['mask-data-in-storage']).toBe(true);
  });

  it('includes AIRS UI-required defaults even when flags not provided for a section', () => {
    const result = buildProfileRequest({
      name: 'Test',
      promptInjection: 'block',
      // No app-protection, agent-protection, data-protection, or latency flags
    });
    const config = getModelConfiguration(result);
    expect(config?.['app-protection']).toEqual({
      'default-url-category': { member: ['malicious'] },
      'url-detected-action': 'block',
    });
    expect(config?.['agent-protection']).toBeUndefined();
    expect(config?.['data-protection']).toEqual({
      'data-leak-detection': { action: '', 'mask-data-inline': false, member: null },
      'database-security': null,
    });
    expect(config?.latency).toEqual({
      'inline-timeout-action': 'block',
      'max-inline-latency': 5,
    });
    expect(config?.['mask-data-in-storage']).toBe(false);
  });
});

describe('buildProfileOverrides', () => {
  it('returns undefined when no flags provided', () => {
    const result = buildProfileOverrides({});
    expect(result).toBeUndefined();
  });

  it('builds partial policy from single flag', () => {
    const result = buildProfileOverrides({ promptInjection: 'alert' });
    expect(result).toBeDefined();
    const mp = result?.['ai-security-profiles']?.[0]?.['model-configuration']?.['model-protection'];
    expect(mp).toContainEqual(
      expect.objectContaining({ name: 'prompt-injection', action: 'alert' }),
    );
  });

  it('builds partial policy from multiple sections', () => {
    const result = buildProfileOverrides({
      promptInjection: 'block',
      agentSecurity: 'alert',
    });
    expect(result).toBeDefined();
    const config = result?.['ai-security-profiles']?.[0]?.['model-configuration'];
    expect(config?.['model-protection']).toBeDefined();
    expect(config?.['agent-protection']).toBeDefined();
  });
});

describe('mergeProfilePolicy', () => {
  const existingPolicy = {
    'ai-security-profiles': [
      {
        'model-type': 'default',
        'model-configuration': {
          'model-protection': [
            { name: 'prompt-injection', action: 'block' },
            { name: 'toxic-content', action: 'high:block, moderate:block' },
            {
              name: 'topic-guardrails',
              action: 'block',
              'topic-list': [
                {
                  action: 'block',
                  topic: [{ topic_id: 'topic-123', topic_name: 'Weapons', revision: 2 }],
                },
              ],
            },
          ],
          'app-protection': {
            'malicious-code-protection': { name: 'malicious-code-detection', action: 'block' },
            'url-detected-action': 'block',
            'allow-url-category': { member: ['dynamic-dns'] },
          },
          'agent-protection': [{ name: 'agent-security', action: 'block' }],
          'data-protection': {
            'data-leak-detection': {
              action: 'block',
              member: [{ text: 'IP Addresses' }],
              'mask-data-inline': true,
            },
            'database-security': [
              { name: 'database-security-create', action: 'block' },
              { name: 'database-security-read', action: 'block' },
            ],
          },
          latency: {
            'inline-timeout-action': 'block',
            'max-inline-latency': 1,
          },
          'mask-data-in-storage': true,
        },
      },
    ],
    'dlp-data-profiles': [{ name: 'DLP Profile 1' }],
  };

  it('returns existing policy when no overrides', () => {
    const result = mergeProfilePolicy(existingPolicy, undefined);
    expect(result).toEqual(existingPolicy);
  });

  it('preserves topic-guardrails when updating other model-protection items', () => {
    const overrides = buildProfileOverrides({ toxicContent: 'high:alert, moderate:allow' });
    const result = mergeProfilePolicy(existingPolicy, overrides);
    const mp = result['ai-security-profiles']?.[0]?.['model-configuration']?.['model-protection'];
    // topic-guardrails must survive
    const tg = mp?.find((item: Record<string, unknown>) => item.name === 'topic-guardrails');
    expect(tg).toBeDefined();
    expect(tg?.['topic-list']).toEqual(
      existingPolicy['ai-security-profiles'][0]['model-configuration']['model-protection'][2][
        'topic-list'
      ],
    );
    // toxic-content updated
    const tc = mp?.find((item: Record<string, unknown>) => item.name === 'toxic-content');
    expect(tc?.action).toBe('high:alert, moderate:allow');
    // prompt-injection preserved
    const pi = mp?.find((item: Record<string, unknown>) => item.name === 'prompt-injection');
    expect(pi?.action).toBe('block');
  });

  it('preserves all config when updating a single section', () => {
    const overrides = buildProfileOverrides({ agentSecurity: 'alert' });
    const result = mergeProfilePolicy(existingPolicy, overrides);
    const config = result['ai-security-profiles']?.[0]?.['model-configuration'];
    // agent-protection updated
    const ap = config?.['agent-protection'];
    expect(ap).toContainEqual(expect.objectContaining({ name: 'agent-security', action: 'alert' }));
    // model-protection preserved (with topic-guardrails)
    expect(config?.['model-protection']).toHaveLength(3);
    // app-protection preserved
    expect(config?.['app-protection']?.['malicious-code-protection']).toBeDefined();
    // data-protection preserved
    expect(config?.['data-protection']?.['data-leak-detection']).toBeDefined();
    // latency preserved
    expect(config?.latency?.['inline-timeout-action']).toBe('block');
    // dlp-data-profiles preserved
    expect(result['dlp-data-profiles']).toEqual([{ name: 'DLP Profile 1' }]);
  });

  it('adds new protection item to existing profile', () => {
    const overrides = buildProfileOverrides({ contextualGrounding: 'block' });
    const result = mergeProfilePolicy(existingPolicy, overrides);
    const mp = result['ai-security-profiles']?.[0]?.['model-configuration']?.['model-protection'];
    const cg = mp?.find((item: Record<string, unknown>) => item.name === 'contextual-grounding');
    expect(cg?.action).toBe('block');
    // existing items preserved
    expect(
      mp?.find((item: Record<string, unknown>) => item.name === 'prompt-injection'),
    ).toBeDefined();
    expect(
      mp?.find((item: Record<string, unknown>) => item.name === 'toxic-content'),
    ).toBeDefined();
    expect(
      mp?.find((item: Record<string, unknown>) => item.name === 'topic-guardrails'),
    ).toBeDefined();
  });

  it('merges app-protection fields without wiping existing', () => {
    const overrides = buildProfileOverrides({ urlAction: 'alert' });
    const result = mergeProfilePolicy(existingPolicy, overrides);
    const ap = result['ai-security-profiles']?.[0]?.['model-configuration']?.['app-protection'];
    // url-detected-action updated
    expect(ap?.['url-detected-action']).toBe('alert');
    // malicious-code-protection preserved
    expect(ap?.['malicious-code-protection']).toBeDefined();
    // allow-url-category preserved
    expect(ap?.['allow-url-category']?.member).toEqual(['dynamic-dns']);
  });

  it('merges database-security items by name', () => {
    const overrides = buildProfileOverrides({ dbSecurityRead: 'alert' });
    const result = mergeProfilePolicy(existingPolicy, overrides);
    const dp = result['ai-security-profiles']?.[0]?.['model-configuration']?.['data-protection'];
    const dbItems = dp?.['database-security'];
    expect(
      dbItems?.find((i: Record<string, unknown>) => i.name === 'database-security-read')?.action,
    ).toBe('alert');
    // create preserved
    expect(
      dbItems?.find((i: Record<string, unknown>) => i.name === 'database-security-create')?.action,
    ).toBe('block');
    // DLP preserved
    expect(dp?.['data-leak-detection']?.action).toBe('block');
  });

  it('handles empty existing policy with overrides', () => {
    const overrides = buildProfileOverrides({ promptInjection: 'block', agentSecurity: 'alert' });
    const result = mergeProfilePolicy(undefined, overrides);
    const config = result['ai-security-profiles']?.[0]?.['model-configuration'];
    const mp = config?.['model-protection'];
    expect(mp).toContainEqual(
      expect.objectContaining({ name: 'prompt-injection', action: 'block' }),
    );
    expect(config?.['agent-protection']).toContainEqual(
      expect.objectContaining({ name: 'agent-security', action: 'alert' }),
    );
  });

  it('preserves mask-data-in-storage when not overridden', () => {
    const overrides = buildProfileOverrides({ promptInjection: 'alert' });
    const result = mergeProfilePolicy(existingPolicy, overrides);
    const config = result['ai-security-profiles']?.[0]?.['model-configuration'];
    expect(config?.['mask-data-in-storage']).toBe(true);
  });

  it('updates latency fields individually', () => {
    const overrides = buildProfileOverrides({ maxInlineLatency: 5 });
    const result = mergeProfilePolicy(existingPolicy, overrides);
    const config = result['ai-security-profiles']?.[0]?.['model-configuration'];
    expect(config?.latency?.['max-inline-latency']).toBe(5);
    // inline-timeout-action preserved
    expect(config?.latency?.['inline-timeout-action']).toBe('block');
  });
});

// --- Helper functions ---

function getModelConfiguration(
  request: Record<string, unknown>,
): Record<string, unknown> | undefined {
  const policy = request.policy as Record<string, unknown> | undefined;
  const aiProfiles = policy?.['ai-security-profiles'] as Record<string, unknown>[] | undefined;
  return aiProfiles?.[0]?.['model-configuration'] as Record<string, unknown> | undefined;
}

function getModelProtection(
  request: Record<string, unknown>,
): Record<string, unknown>[] | undefined {
  return getModelConfiguration(request)?.['model-protection'] as
    | Record<string, unknown>[]
    | undefined;
}

function getAppProtection(request: Record<string, unknown>): Record<string, unknown> | undefined {
  return getModelConfiguration(request)?.['app-protection'] as Record<string, unknown> | undefined;
}
