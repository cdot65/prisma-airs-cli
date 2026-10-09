import { CreateSecurityProfileRequestSchema, type Policy } from '@cdot65/prisma-airs-sdk';
import {
  PROFILE_DIRECTIONS,
  profileProtectionLocations,
} from '../../../src/airs/profile-policy.js';
import { compareRuntimePolicies } from '../../../src/backup/runtime-policy.js';
import {
  buildProfileOverrides,
  buildProfileRequest,
  mergeProfilePolicy,
} from '../../../src/cli/builders/profile-builder.js';
import fixture from '../../fixtures/directional-security-profile.json';
import { required } from '../../helpers/required.js';

function policy(): Policy {
  return structuredClone(fixture.policy);
}
describe('directional profile flags and preservation', () => {
  it('replays the complete sanitized POST fixture with omissions and explicit false intact', () => {
    expect(CreateSecurityProfileRequestSchema.parse(fixture)).toEqual(fixture);
    expect(
      profileProtectionLocations(fixture.policy)
        .filter((location) => location.active)
        .map((location) => location.direction),
    ).toEqual(PROFILE_DIRECTIONS);
  });
  it.each(
    PROFILE_DIRECTIONS,
  )('merges detectors inside %s while preserving every other direction and nested setting', (direction) => {
    const base = policy();
    const before = structuredClone(base);
    const result = mergeProfilePolicy(
      base,
      buildProfileOverrides({
        direction,
        toxicContent: 'high:alert, moderate:block',
        maliciousCode: 'alert',
        dlpAction: 'allow',
        dbSecurityRead: 'alert',
      }),
    );
    const ai = required(result['ai-security-profiles'])[0];
    const configs = required(ai['content-type-configurations']);
    for (const other of PROFILE_DIRECTIONS.filter((value) => value !== direction))
      expect(configs[other]).toEqual(
        required(required(before['ai-security-profiles'])[0]['content-type-configurations'])[other],
      );
    const selected = required(configs[direction]);
    expect(
      required(selected['model-protection']).find((item) => item.name === 'toxic-content'),
    ).toMatchObject({
      action: 'high:alert, moderate:block',
      'severity-by-confidence': { high: 'medium', moderate: 'low' },
    });
    expect(required(selected['app-protection'])['malicious-code-protection']).toMatchObject({
      name: 'malicious-code',
      action: 'alert',
      severity: 'high',
    });
    expect(required(required(selected['data-protection'])['data-leak-detection']).member).toEqual(
      required(
        required(
          required(
            required(required(before['ai-security-profiles'])[0]['content-type-configurations'])[
              direction
            ],
          )['data-protection'],
        )['data-leak-detection'],
      ).member,
    );
    expect(ai['model-configuration']).toEqual(
      required(before['ai-security-profiles'])[0]['model-configuration'],
    );
    expect(base).toEqual(before);
  });
  it('changes masking without needing to replace the DLP action or members', () => {
    const result = mergeProfilePolicy(
      policy(),
      buildProfileOverrides({ direction: 'response', maskDataInline: false }),
    );
    expect(
      required(
        required(
          required(required(result['ai-security-profiles'])[0]['content-type-configurations'])
            .response,
        )['data-protection'],
      )['data-leak-detection'],
    ).toEqual({
      ...fixture.policy['ai-security-profiles'][0]['content-type-configurations'].response[
        'data-protection'
      ]['data-leak-detection'],
      'mask-data-inline': false,
    });
  });
  it('keeps global-only edits separate and retains future direction fields', () => {
    const base = policy();
    required(required(base['ai-security-profiles'])[0]['content-type-configurations'])[
      'future-direction'
    ] = {
      enabled: true,
    };
    const result = mergeProfilePolicy(
      base,
      buildProfileOverrides({
        maxInlineLatency: 12,
        maskDataInStorage: false,
        enableFullConversationInspection: false,
      }),
    );
    expect(required(result['ai-security-profiles'])[0]['content-type-configurations']).toEqual(
      required(base['ai-security-profiles'])[0]['content-type-configurations'],
    );
    expect(required(result['ai-security-profiles'])[0]['model-configuration']).toMatchObject({
      'mask-data-in-storage': false,
      'enable-full-conversation-inspection': false,
      latency: { 'inline-timeout-action': 'block', 'max-inline-latency': 12 },
    });
  });
  it('global edits do not insert a missing selected direction', () => {
    const base = policy();
    delete required(required(base['ai-security-profiles'])[0]['content-type-configurations'])
      .response;
    const result = mergeProfilePolicy(
      base,
      buildProfileOverrides({ direction: 'response', maskDataInStorage: true }),
    );
    expect(required(result['ai-security-profiles'])[0]['content-type-configurations']).toEqual(
      required(base['ai-security-profiles'])[0]['content-type-configurations'],
    );
  });
  it('creates only the selected direction and shared settings', () => {
    const request = buildProfileRequest({
      name: 'Directional',
      direction: 'tool-call',
      promptInjection: 'block',
      enableFullConversationInspection: false,
    });
    expect(CreateSecurityProfileRequestSchema.parse(request)).toEqual(request);
    expect(required(required(request.policy)['ai-security-profiles'])[0]).toEqual({
      'model-type': 'default',
      'content-type-mode': 'per_content_type',
      'model-configuration': { 'enable-full-conversation-inspection': false },
      'content-type-configurations': {
        'tool-call': { 'model-protection': [{ name: 'prompt-injection', action: 'block' }] },
      },
    });
  });
  it('rejects ambiguous directions, layout conversion, and multiple entries', () => {
    expect(() =>
      mergeProfilePolicy(policy(), buildProfileOverrides({ promptInjection: 'alert' })),
    ).toThrow(/--direction/);
    const legacy = { 'ai-security-profiles': [{ 'model-configuration': {} }] };
    expect(() =>
      mergeProfilePolicy(
        legacy,
        buildProfileOverrides({ direction: 'response', toxicContent: 'alert' }),
      ),
    ).toThrow(/legacy/);
    expect(() => mergeProfilePolicy(legacy, undefined, { direction: 'response' })).toThrow(
      /legacy/,
    );
    const multi = policy();
    required(multi['ai-security-profiles']).unshift({
      'model-type': 'other',
      'model-configuration': { 'model-protection': [{ name: 'other', action: 'allow' }] },
    });
    const overrides = buildProfileOverrides({ direction: 'response', toxicContent: 'alert' });
    expect(() => mergeProfilePolicy(multi, overrides)).toThrow(/--ai-profile-index/);
    const result = mergeProfilePolicy(multi, overrides, { aiProfileIndex: 1 });
    expect(required(result['ai-security-profiles'])[0]).toEqual(
      required(multi['ai-security-profiles'])[0],
    );
  });
  it('refuses protection edits to an absent direction and empty directional flag creation', () => {
    const base = policy();
    delete required(required(base['ai-security-profiles'])[0]['content-type-configurations'])
      .response;
    expect(() =>
      mergeProfilePolicy(
        base,
        buildProfileOverrides({ direction: 'response', toxicContent: 'alert' }),
      ),
    ).toThrow(/Selected direction is absent/);
    expect(() => buildProfileRequest({ name: 'Empty', direction: 'response' })).toThrow(
      /at least one/,
    );
    expect(() =>
      mergeProfilePolicy({}, buildProfileOverrides({ toxicContent: 'alert' }), {
        aiProfileIndex: 0,
      }),
    ).toThrow(/existing AI profile/);
    const request = buildProfileRequest({
      name: 'Directional',
      direction: 'prompt',
      maliciousCode: 'block',
    });
    expect(
      required(
        required(request.policy?.['ai-security-profiles'])[0]['content-type-configurations']
          ?.prompt,
      )['app-protection']?.['malicious-code-protection']?.name,
    ).toBe('malicious-code');
  });

  it('rejects malformed selectors and global types locally', () => {
    expect(() => buildProfileOverrides({ direction: 'invalid' as never })).toThrow(/--direction/);
    expect(() => buildProfileOverrides({ aiProfileIndex: -1 })).toThrow(/--ai-profile-index/);
    expect(() => buildProfileOverrides({ maxInlineLatency: 1.5 })).toThrow();
  });
  it('compares directions strictly without extending legacy server-default allowances', () => {
    const base = policy();
    expect(compareRuntimePolicies(base, policy()).matches).toBe(true);
    const changed = policy();
    required(
      required(
        required(required(changed['ai-security-profiles'])[0]['content-type-configurations'])
          .response,
      )['model-protection'],
    )[0].action = 'allow';
    expect(compareRuntimePolicies(base, changed).matches).toBe(false);
    const missing = policy();
    delete required(required(missing['ai-security-profiles'])[0]['content-type-configurations'])[
      'tool-call'
    ];
    expect(compareRuntimePolicies(base, missing).matches).toBe(false);
    const omitted = policy();
    delete required(
      required(
        required(required(omitted['ai-security-profiles'])[0]['content-type-configurations'])
          .prompt,
      )['model-protection'],
    )[0].severity;
    expect(compareRuntimePolicies(omitted, base)).toMatchObject({
      matches: false,
      serverDefaults: [],
    });
    const unknown = policy();
    required(
      required(required(unknown['ai-security-profiles'])[0]['content-type-configurations'])
        .response,
    ).future = false;
    expect(compareRuntimePolicies(base, unknown).matches).toBe(false);
  });
});
