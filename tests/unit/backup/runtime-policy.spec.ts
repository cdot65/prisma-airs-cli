import type { SecurityProfile } from '@cdot65/prisma-airs-sdk';
import { compareRuntimePolicies } from '../../../src/backup/runtime-policy.js';

function fixture() {
  const source = {
    'ai-security-profiles': [
      {
        'model-configuration': {
          'data-protection': {
            'database-security': ['create', 'read', 'update', 'delete'].map((name) => ({
              name: `database-security-${name}`,
              action: 'block',
            })),
            'source-code-detection': { action: 'block' },
          },
          'app-protection': {
            'url-detected-action': 'block',
            'malicious-code-protection': { name: 'malicious-code', action: 'block' },
          },
          'model-protection': [
            { name: 'prompt-injection', action: 'block' },
            { name: 'toxic-content', action: 'high:block, moderate:block' },
          ],
          'agent-protection': [{ name: 'agent-security', action: 'block' }],
        },
      },
    ],
  };
  const original = source['ai-security-profiles'][0]['model-configuration'];
  const model = {
    'data-protection': {
      'database-security': original['data-protection']['database-security'].map((item, i) => ({
        ...item,
        severity: ['medium', 'low', 'medium', 'high'][i],
      })),
      'source-code-detection': { action: 'block', severity: 'high' },
    },
    'app-protection': {
      ...original['app-protection'],
      'malicious-code-protection': { name: 'malicious-code', action: 'block', severity: 'high' },
      'url-detected-severity': 'low',
    },
    'model-protection': [
      { name: 'prompt-injection', action: 'block', severity: 'medium' },
      {
        name: 'toxic-content',
        action: 'high:block, moderate:block',
        'severity-by-confidence': { high: 'medium', moderate: 'low' },
      },
    ] as [
      { name: string; action: string; severity: string },
      { name: string; action: string; 'severity-by-confidence': Record<string, string> },
    ],
    'agent-protection': [{ name: 'agent-security', action: 'block', severity: 'medium' }],
  };
  const target = { 'ai-security-profiles': [{ 'model-configuration': model }] };
  return { source, target, model };
}

describe('directional Runtime policy verification', () => {
  it('accepts observed blocked contextual-grounding default but rejects overrides and action drift', () => {
    const policy = (detector: Record<string, unknown>) => ({
      'ai-security-profiles': [
        {
          'model-configuration': { 'model-protection': [detector] },
        },
      ],
    });
    const detector = { name: 'contextual-grounding', action: 'block' };
    const source = policy(detector),
      target = policy({ ...detector, severity: 'medium' });
    expect(compareRuntimePolicies(source, target).serverDefaults).toHaveLength(1);
    expect(compareRuntimePolicies(source, target).matches).toBe(true);
    expect(compareRuntimePolicies(policy({ ...detector, severity: 'high' }), target).matches).toBe(
      false,
    );
    expect(
      compareRuntimePolicies(source, policy({ ...detector, action: 'allow', severity: 'medium' }))
        .matches,
    ).toBe(false);
    expect(compareRuntimePolicies(source, policy({ ...detector, severity: 'high' })).matches).toBe(
      false,
    );
  });
  function topicPolicy(explicit = false, action = 'block', name = 'topic-guardrails') {
    return {
      'ai-security-profiles': [
        {
          'model-configuration': {
            'model-protection': [
              {
                name,
                ...(explicit ? { severity: 'medium' } : {}),
                'topic-list': [
                  {
                    action,
                    topic: [
                      {
                        topic_id: 'mapped-destination',
                        topic_name: 'example',
                        revision: 1,
                        ...(explicit ? { severity: 'medium' } : {}),
                      },
                    ],
                  },
                ],
              },
            ],
          },
        },
      ],
    };
  }
  it('accepts only omitted medium severities on topic guardrails and matching blocked references', () => {
    const source = topicPolicy(),
      target = topicPolicy(true);
    const before = structuredClone({ source, target });
    const result = compareRuntimePolicies(source, target);
    expect(result.matches).toBe(true);
    expect(result.serverDefaults).toHaveLength(2);
    expect({ source, target }).toEqual(before);
    expect(compareRuntimePolicies(target, source).matches).toBe(false);
    expect(compareRuntimePolicies(target, structuredClone(target)).matches).toBe(true);
    expect(
      compareRuntimePolicies(topicPolicy(false, 'allow'), topicPolicy(true, 'allow')).matches,
    ).toBe(false);
    expect(
      compareRuntimePolicies(
        topicPolicy(false, 'block', 'other'),
        topicPolicy(true, 'block', 'other'),
      ).matches,
    ).toBe(false);
  });
  it.each([
    'topic_id',
    'topic_name',
    'revision',
    'severity',
    'action',
    'explicit',
  ])('rejects topic %s drift', (key) => {
    const source = topicPolicy(),
      target = topicPolicy(true);
    const detector =
      target['ai-security-profiles'][0]['model-configuration']['model-protection'][0];
    const topic = detector['topic-list'][0].topic[0];
    if (key === 'action') detector['topic-list'][0].action = 'allow';
    else if (key === 'explicit')
      Object.assign(
        source['ai-security-profiles'][0]['model-configuration']['model-protection'][0][
          'topic-list'
        ][0].topic[0],
        { severity: 'high' },
      );
    else Object.assign(topic, { [key]: key === 'revision' ? 2 : 'changed' });
    expect(compareRuntimePolicies(source, target).matches).toBe(false);
  });
  function categories(explicit = false) {
    return {
      'ai-security-profiles': [0, 1].map(() => ({
        'model-configuration': {
          'model-protection': [
            { name: 'prompt-injection', action: 'block' },
            {
              name: 'toxic-content',
              action: '',
              'toxic-category-list': ['hate', 'self-harm'].map((category) => ({
                category,
                action: 'high:allow, moderate:allow',
                ...(explicit
                  ? { 'severity-by-confidence': { high: 'medium', moderate: 'low' } }
                  : {}),
              })),
            },
          ],
        },
      })),
    };
  }
  it('accepts omitted category confidence defaults across multiple models, and reports paths', () => {
    const source = categories();
    const target = categories(true);
    const before = structuredClone({ source, target });
    const result = compareRuntimePolicies(source, target);
    expect(result.matches).toBe(true);
    expect(result.serverDefaults).toHaveLength(4);
    expect(result.serverDefaults[3]).toContain(
      'ai-security-profiles[1].model-configuration.model-protection[1].toxic-category-list[1].severity-by-confidence',
    );
    expect({ source, target }).toEqual(before);
    expect(compareRuntimePolicies(target, source).matches).toBe(false);
  });
  it.each([
    'category',
    'action',
    'name',
    'explicit',
    'extra',
    'unexpected-default',
    'misplaced',
  ])('rejects category %s drift', (mutation) => {
    const source = categories();
    const target = categories(true);
    const detector =
      target['ai-security-profiles'][0]['model-configuration']['model-protection'][1];
    const category = detector['toxic-category-list']?.[0];
    if (!category) throw new Error('Invalid fixture');
    if (mutation === 'category') category.category = 'other';
    if (mutation === 'action') category.action = 'high:block, moderate:block';
    if (mutation === 'name') detector.name = 'other-detector';
    if (mutation === 'explicit')
      Object.assign(
        source['ai-security-profiles'][0]['model-configuration']['model-protection'][1][
          'toxic-category-list'
        ]?.[0] ?? {},
        { 'severity-by-confidence': { high: 'high', moderate: 'low' } },
      );
    if (mutation === 'extra')
      Object.assign(category['severity-by-confidence'] ?? {}, { extra: 'low' });
    if (mutation === 'unexpected-default')
      Object.assign(category['severity-by-confidence'] ?? {}, { high: 'high' });
    if (mutation === 'misplaced') {
      detector.name = 'other-detector';
      source['ai-security-profiles'][0]['model-configuration']['model-protection'][1].name =
        'other-detector';
    }
    expect(compareRuntimePolicies(source, target).matches).toBe(false);
  });
  it('preserves explicitly equal category severities but refuses extra or missing categories', () => {
    const source = categories(true);
    const target = categories(true);
    expect(compareRuntimePolicies(source, target).matches).toBe(true);
    target['ai-security-profiles'][0]['model-configuration']['model-protection'][1][
      'toxic-category-list'
    ]?.pop();
    expect(compareRuntimePolicies(source, target).matches).toBe(false);
    expect(compareRuntimePolicies(target, source).matches).toBe(false);
  });
  it('allows only the observed empty default URL category addition', () => {
    const source = {
      'ai-security-profiles': [{ 'model-configuration': { 'app-protection': {} } }],
    };
    const policy = (category: unknown) =>
      ({
        'ai-security-profiles': [
          { 'model-configuration': { 'app-protection': { 'default-url-category': category } } },
        ],
      }) as SecurityProfile['policy'];
    expect(compareRuntimePolicies(source, policy({ member: null })).matches).toBe(true);
    for (const value of [{ member: ['malicious'] }, { member: null, unknown: true }, null, []])
      expect(compareRuntimePolicies(source, policy(value)).matches).toBe(false);
    expect(
      compareRuntimePolicies(policy({ member: ['malicious'] }), policy({ member: null })).matches,
    ).toBe(false);
  });
  it('accepts and reports exactly the ten observed omitted fields, without input mutation', () => {
    const { source, target } = fixture();
    const before = structuredClone({ source, target });
    const result = compareRuntimePolicies(source, target as SecurityProfile['policy']);
    expect(result.matches).toBe(true);
    expect(result.serverDefaults).toHaveLength(10);
    expect({ source, target }).toEqual(before);
    expect(compareRuntimePolicies(target as SecurityProfile['policy'], source).matches).toBe(false);
  });
  it('requires equality of explicit severities including null and confidence maps', () => {
    const { target } = fixture();
    expect(
      compareRuntimePolicies(
        target as SecurityProfile['policy'],
        structuredClone(target) as SecurityProfile['policy'],
      ).matches,
    ).toBe(true);
    for (const explicit of ['high', null, '']) {
      const source = structuredClone(target);
      Object.assign(
        source['ai-security-profiles'][0]['model-configuration']['model-protection'][0],
        { severity: explicit },
      );
      expect(
        compareRuntimePolicies(
          source as SecurityProfile['policy'],
          target as SecurityProfile['policy'],
        ).matches,
      ).toBe(false);
    }
    const source = structuredClone(target);
    source['ai-security-profiles'][0]['model-configuration']['model-protection'][1][
      'severity-by-confidence'
    ].high = 'high';
    expect(
      compareRuntimePolicies(
        source as SecurityProfile['policy'],
        target as SecurityProfile['policy'],
      ).matches,
    ).toBe(false);
  });
  it.each(['severity', 'action', 'name', 'unknown'])('rejects unrecognized %s changes', (key) => {
    const { source, target, model } = fixture();
    Object.assign(model['model-protection'][0], { [key]: 'unexpected' });
    expect(compareRuntimePolicies(source, target as SecurityProfile['policy']).matches).toBe(false);
  });
  it('rejects extra confidence settings, extra detectors, wrong shapes and misplaced defaults', () => {
    const { source, target, model } = fixture();
    model['model-protection'][1]['severity-by-confidence'].extra = 'low';
    expect(compareRuntimePolicies(source, target as SecurityProfile['policy']).matches).toBe(false);
    expect(compareRuntimePolicies(source, undefined).matches).toBe(false);
    expect(compareRuntimePolicies({}, { severity: 'medium' }).matches).toBe(false);
    expect(compareRuntimePolicies({ 'ai-security-profiles': [] }, source).matches).toBe(false);
    expect(compareRuntimePolicies({ future: [] }, { future: {} }).matches).toBe(false);
  });
  it('allows only an added null database-security section, never removal or enabled rules', () => {
    const source = {
      'ai-security-profiles': [{ 'model-configuration': { 'data-protection': {} } }],
    };
    const target = {
      'ai-security-profiles': [
        { 'model-configuration': { 'data-protection': { 'database-security': null } } },
      ],
    };
    expect(compareRuntimePolicies(source, target).matches).toBe(true);
    expect(compareRuntimePolicies(target, source).matches).toBe(false);
    expect(
      compareRuntimePolicies(source, fixture().target as SecurityProfile['policy']).matches,
    ).toBe(false);
  });
});
