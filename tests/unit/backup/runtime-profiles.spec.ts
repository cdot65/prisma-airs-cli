import type { CustomTopic, SecurityProfile } from '@cdot65/prisma-airs-sdk';
import {
  backupRuntimeProfiles,
  type ProfileTransferApi,
  planRuntimeProfilesRestore,
  type RuntimeProfilesBackup,
  restoreRuntimeProfiles,
} from '../../../src/backup/runtime-profiles.js';

const sourceTopic: CustomTopic = {
  topic_id: 'source-topic',
  topic_name: 'Restricted',
  revision: 3,
  description: 'Sensitive business topics',
  examples: ['Example'],
  active: true,
};
const profile = (name = 'Production'): SecurityProfile => ({
  profile_id: `source-${name}`,
  profile_name: name,
  tsg_id: '100',
  revision: 8,
  active: true,
  created_by: 'source-user',
  csp_id: 'source-csp',
  policy: {
    'ai-security-profiles': [
      {
        'model-configuration': {
          'model-protection': [
            {
              name: 'topic-guardrail',
              'topic-list': [
                {
                  action: 'block',
                  topic: [
                    {
                      topic_id: 'source-topic',
                      topic_name: sourceTopic.topic_name,
                      revision: 3,
                    },
                  ],
                },
              ],
            },
          ],
          'mask-data-in-storage': true,
        },
      },
    ],
    'dlp-data-profiles': [],
    future_setting: { enabled: true },
  },
});
const archive = (): RuntimeProfilesBackup => ({
  version: 1,
  resourceType: 'runtime-security-profiles',
  exportedAt: '2026-09-08T00:00:00Z',
  source: { tsgId: '100' },
  profiles: [profile()],
  topics: [structuredClone(sourceTopic)],
});

function memoryApi() {
  const state = { profiles: [] as SecurityProfile[], topics: [] as CustomTopic[] };
  const api = {
    profiles: {
      list: vi.fn(async () => ({ ai_profiles: structuredClone(state.profiles), next_offset: 0 })),
      create: vi.fn(async (body) => {
        const p = {
          ...structuredClone(body),
          profile_id: `dest-${state.profiles.length}`,
          revision: 1,
          tsg_id: '200',
        };
        state.profiles.push(p);
        return structuredClone(p);
      }),
      update: vi.fn(async (id, body) => {
        const at = state.profiles.findIndex((p) => p.profile_id === id);
        state.profiles[at] = {
          ...structuredClone(body),
          profile_id: id,
          revision: (state.profiles[at].revision ?? 0) + 1,
          tsg_id: '200',
        };
        return structuredClone(state.profiles[at]);
      }),
    },
    topics: {
      list: vi.fn(async () => ({ custom_topics: structuredClone(state.topics), next_offset: 0 })),
      create: vi.fn(async (body) => {
        const t = {
          ...structuredClone(body),
          topic_id: `dest-topic-${state.topics.length}`,
          revision: 1,
        };
        state.topics.push(t);
        return structuredClone(t);
      }),
    },
    dataProfiles: {
      list: vi.fn(async () => ({
        content: [{ id: 'dest-dlp', name: 'Destination DLP', version: 5 }],
        last: true,
      })),
    },
  } satisfies ProfileTransferApi;
  return { api, state };
}

function withDlp(): RuntimeProfilesBackup {
  const input = archive();
  const policy = input.profiles[0].policy;
  if (!policy) throw new Error('Invalid test fixture');
  policy['dlp-data-profiles'] = [
    {
      name: 'Source DLP',
      uuid: 'source-dlp',
      id: 'source-dlp',
      version: '2',
      rule1: { action: 'block' },
    },
  ];
  const model = policy['ai-security-profiles']?.[0]['model-configuration'];
  if (!model) throw new Error('Invalid test fixture');
  model['data-protection'] = {
    'data-leak-detection': {
      action: 'block',
      member: [{ text: 'Source DLP', id: 'source-dlp', version: '2' }],
    },
  };
  return input;
}

describe('Runtime profile backup', () => {
  it('exports latest policies and exact topic revisions, preserving unknown settings', async () => {
    const { api, state } = memoryApi();
    state.profiles = [profile(), { ...profile(), revision: 1 }];
    state.topics = [sourceTopic];
    const result = await backupRuntimeProfiles(api, '100');
    expect(result.profiles).toEqual([profile()]);
    expect(result.topics).toEqual([sourceTopic]);
    expect(api.profiles.list).toHaveBeenCalledWith({ offset: 0, limit: 100, latest: true });
    expect(api.profiles.create).not.toHaveBeenCalled();
  });
  it.each(['Production', 'source-Production'])('selects by name or ID: %s', async (selector) => {
    const { api, state } = memoryApi();
    state.profiles = [profile(), profile('Other')];
    state.topics = [sourceTopic];
    expect((await backupRuntimeProfiles(api, '100', { profile: selector })).profiles).toHaveLength(
      1,
    );
  });
  it('does not fetch unreferenced topics', async () => {
    const { api, state } = memoryApi();
    state.profiles = [{ ...profile(), policy: {} }];
    expect((await backupRuntimeProfiles(api, '100')).topics).toEqual([]);
    expect(api.topics.list).not.toHaveBeenCalled();
  });
  it('walks complete pages', async () => {
    const { api, state } = memoryApi();
    state.topics = [sourceTopic];
    api.profiles.list
      .mockResolvedValueOnce({ ai_profiles: [profile()], next_offset: 100 })
      .mockResolvedValueOnce({ ai_profiles: [profile('Other')], next_offset: 0 });
    expect((await backupRuntimeProfiles(api, '100')).profiles).toHaveLength(2);
    expect(api.profiles.list).toHaveBeenLastCalledWith({ offset: 100, limit: 100, latest: true });
  });
  it.each([
    'absent',
    'revision',
    'duplicate',
  ])('rejects unavailable exact topics: %s', async (mode) => {
    const { api, state } = memoryApi();
    state.profiles = [profile()];
    state.topics =
      mode === 'absent'
        ? []
        : mode === 'revision'
          ? [{ ...sourceTopic, revision: 4 }]
          : [sourceTopic, sourceTopic];
    await expect(backupRuntimeProfiles(api, '100')).rejects.toThrow('exact referenced topic');
  });
  it('refuses incomplete, repeated and stalled inventories', async () => {
    const { api } = memoryApi();
    api.profiles.list.mockResolvedValue({ ai_profiles: [profile()], next_offset: 100 });
    await expect(backupRuntimeProfiles(api, '100', { maxPages: 1 })).rejects.toThrow('incomplete');
    await expect(backupRuntimeProfiles(api, '100')).rejects.toThrow('did not advance');
    api.profiles.list.mockResolvedValue({ ai_profiles: [profile(), profile()], next_offset: 0 });
    await expect(backupRuntimeProfiles(api, '100')).rejects.toThrow('repeated');
  });
  it.each([0, -1, 1.5, 1001])('rejects invalid page budget %s', async (maxPages) => {
    const { api } = memoryApi();
    await expect(backupRuntimeProfiles(api, '100', { maxPages })).rejects.toThrow('maxPages');
    expect(api.profiles.list).not.toHaveBeenCalled();
  });
  it('rejects missing, ambiguous, wrong-tenant, and policy-less profiles', async () => {
    const { api, state } = memoryApi();
    await expect(backupRuntimeProfiles(api, '100')).rejects.toThrow('No matching');
    state.profiles = [profile(), { ...profile(), profile_id: 'different' }];
    await expect(backupRuntimeProfiles(api, '100')).rejects.toThrow('Ambiguous');
    state.profiles = [{ ...profile(), tsg_id: '200' }];
    await expect(backupRuntimeProfiles(api, '100')).rejects.toThrow('tenant identity');
    state.profiles = [{ ...profile(), policy: undefined }];
    await expect(backupRuntimeProfiles(api, '100')).rejects.toThrow('no policy');
  });
});

describe('Runtime profile cross-tenant restore', () => {
  it('refuses DLP bindings changed between planning and execution', async () => {
    const { api } = memoryApi();
    const plan = await planRuntimeProfilesRestore(api, withDlp(), '200', {
      dlpMap: { 'Source DLP': 'Destination DLP' },
    });
    api.dataProfiles.list.mockResolvedValue({
      content: [{ id: 'replaced', name: 'Destination DLP', version: 5 }],
      last: true,
    });
    expect((await restoreRuntimeProfiles(api, plan)).error).toContain('DLP profile changed');
    expect(api.topics.create).not.toHaveBeenCalled();
  });
  it.each([
    'objectId',
    'extra_uuid',
  ])('refuses unexpected tenant references even inside known topic objects: %s', async (key) => {
    const { api } = memoryApi();
    const input = archive();
    const topic =
      input.profiles[0].policy?.['ai-security-profiles']?.[0]['model-configuration']?.[
        'model-protection'
      ]?.[0]['topic-list']?.[0].topic?.[0];
    if (!topic) throw new Error('Invalid test fixture');
    topic[key] = 'unmapped-source-id';
    await expect(planRuntimeProfilesRestore(api, input, '200')).rejects.toThrow(
      'Unsupported cross-tenant',
    );
    expect(api.profiles.list).not.toHaveBeenCalled();
  });
  it('plans without mutations then creates fresh IDs and verifies exact policies', async () => {
    const { api, state } = memoryApi();
    const input = archive();
    const original = structuredClone(input);
    const plan = await planRuntimeProfilesRestore(api, input, '200', { namePrefix: 'migrated-' });
    expect(api.profiles.create).not.toHaveBeenCalled();
    expect(api.topics.create).not.toHaveBeenCalled();
    const result = await restoreRuntimeProfiles(api, plan);
    expect(result.complete).toBe(true);
    expect(input).toEqual(original);
    expect(state.profiles[0].profile_name).toBe('migrated-Production');
    expect(Object.keys(api.profiles.create.mock.calls[0][0]).sort()).toEqual([
      'active',
      'policy',
      'profile_name',
    ]);
    expect(Object.keys(api.topics.create.mock.calls[0][0]).sort()).toEqual([
      'active',
      'description',
      'examples',
      'topic_name',
    ]);
    expect(JSON.stringify(state.profiles[0].policy)).toContain('dest-topic-0');
    expect(JSON.stringify(state.profiles[0].policy)).not.toContain('source-topic');
    expect(state.profiles[0].policy?.future_setting).toEqual({ enabled: true });
  });
  it('requires an explicit DLP mapping and binds destination IDs/versions', async () => {
    const { api, state } = memoryApi();
    const input = withDlp();
    await expect(planRuntimeProfilesRestore(api, input, '200')).rejects.toThrow('--dlp-map');
    expect(api.topics.create).not.toHaveBeenCalled();
    const plan = await planRuntimeProfilesRestore(api, input, '200', {
      dlpMap: { 'Source DLP': 'Destination DLP' },
    });
    expect((await restoreRuntimeProfiles(api, plan)).complete).toBe(true);
    expect(state.profiles[0].policy?.['dlp-data-profiles']?.[0]).toMatchObject({
      name: 'Destination DLP',
      uuid: 'dest-dlp',
      id: 'dest-dlp',
      version: '5',
      rule1: { action: 'block' },
    });
    expect(JSON.stringify(state.profiles[0].policy)).not.toContain('source-dlp');
  });
  it('rejects unknown DLP versions before any write', async () => {
    const { api } = memoryApi();
    api.dataProfiles.list.mockResolvedValue({
      content: [
        { id: 'dest-dlp', name: 'Destination DLP', version: undefined as unknown as number },
      ],
      last: true,
    });
    await expect(
      planRuntimeProfilesRestore(api, withDlp(), '200', {
        dlpMap: { 'Source DLP': 'Destination DLP' },
      }),
    ).rejects.toThrow('version is unknown');
    expect(api.topics.create).not.toHaveBeenCalled();
  });
  it('does not transplant an undocumented ID', async () => {
    const { api } = memoryApi();
    const input = archive();
    input.profiles[0].policy = {
      ...input.profiles[0].policy,
      future: { object_id: 'source-only' },
    };
    await expect(planRuntimeProfilesRestore(api, input, '200')).rejects.toThrow(
      'Unsupported cross-tenant',
    );
    expect(api.profiles.list).not.toHaveBeenCalled();
  });
  it('reuses identical topics but refuses mismatched or ambiguous shared topics', async () => {
    const { api, state } = memoryApi();
    state.topics = [{ ...sourceTopic, topic_id: 'dest-existing', revision: 9 }];
    const plan = await planRuntimeProfilesRestore(api, archive(), '200');
    expect((await restoreRuntimeProfiles(api, plan)).complete).toBe(true);
    expect(api.topics.create).not.toHaveBeenCalled();
    state.profiles = [];
    state.topics[0].description = 'Different';
    await expect(planRuntimeProfilesRestore(api, archive(), '200')).rejects.toThrow(
      'topic differs',
    );
    state.topics.push({ ...sourceTopic, topic_id: 'other' });
    await expect(planRuntimeProfilesRestore(api, archive(), '200')).rejects.toThrow(
      'Ambiguous destination topic',
    );
  });
  it('defaults to conflict refusal; skip never creates dependencies', async () => {
    const { api, state } = memoryApi();
    state.profiles = [{ ...profile(), tsg_id: '200', profile_id: 'destination-profile' }];
    await expect(planRuntimeProfilesRestore(api, archive(), '200')).rejects.toThrow(
      'already exists',
    );
    const plan = await planRuntimeProfilesRestore(api, archive(), '200', { onConflict: 'skip' });
    expect(plan.topics).toEqual([]);
    expect((await restoreRuntimeProfiles(api, plan)).profiles[0].action).toBe('skipped');
    expect(api.profiles.create).not.toHaveBeenCalled();
    expect(api.topics.create).not.toHaveBeenCalled();
  });
  it('explicit update uses only the destination profile identity', async () => {
    const { api, state } = memoryApi();
    state.profiles = [{ ...profile(), tsg_id: '200', profile_id: 'destination-profile' }];
    const plan = await planRuntimeProfilesRestore(api, archive(), '200', { onConflict: 'update' });
    expect((await restoreRuntimeProfiles(api, plan)).complete).toBe(true);
    expect(api.profiles.update).toHaveBeenCalledWith(
      'destination-profile',
      expect.objectContaining({ profile_name: 'Production' }),
    );
    expect(api.profiles.create).not.toHaveBeenCalled();
  });
  it('refuses stale plans before creating topics', async () => {
    const { api, state } = memoryApi();
    const plan = await planRuntimeProfilesRestore(api, archive(), '200');
    state.profiles = [{ ...profile(), tsg_id: '200' }];
    const result = await restoreRuntimeProfiles(api, plan);
    expect(result.complete).toBe(false);
    expect(result.error).toContain('changed after planning');
    expect(api.topics.create).not.toHaveBeenCalled();
  });
  it('rechecks newly appeared topics and changed policies just before writes', async () => {
    const { api, state } = memoryApi();
    const plan = await planRuntimeProfilesRestore(api, archive(), '200');
    state.topics = [{ ...sourceTopic, topic_id: 'concurrent' }];
    expect((await restoreRuntimeProfiles(api, plan)).error).toContain('topic changed');
    expect(api.topics.create).not.toHaveBeenCalled();
  });
  it('reports partial writes and sanitized API errors without automatic retries', async () => {
    const { api } = memoryApi();
    const plan = await planRuntimeProfilesRestore(api, archive(), '200');
    api.profiles.create.mockRejectedValue(
      Object.assign(new Error('SECRET response body'), { statusCode: 503 }),
    );
    const result = await restoreRuntimeProfiles(api, plan);
    expect(result.complete).toBe(false);
    expect(result.topics).toHaveLength(1);
    expect(result.error).toContain('HTTP 503');
    expect(result.error).not.toContain('SECRET');
    expect(api.profiles.create).toHaveBeenCalledTimes(1);
  });
  it('fails verification when the service changes the policy', async () => {
    const { api, state } = memoryApi();
    const plan = await planRuntimeProfilesRestore(api, archive(), '200');
    api.profiles.create.mockImplementation(async (body) => {
      const p = { ...body, profile_id: 'bad', revision: 1, tsg_id: '200', policy: {} };
      state.profiles.push(p);
      return p;
    });
    const result = await restoreRuntimeProfiles(api, plan);
    expect(result.complete).toBe(false);
    expect(result.error).toContain('did not verify');
    expect(result.profiles).toHaveLength(1);
  });
  it.each([
    {},
    { ...archive(), version: 2 },
    { ...archive(), source: { tsgId: 'wrong' } },
  ])('refuses invalid archives before API calls', async (input) => {
    const { api } = memoryApi();
    await expect(planRuntimeProfilesRestore(api, input, '200')).rejects.toThrow();
    expect(api.profiles.list).not.toHaveBeenCalled();
  });
});
