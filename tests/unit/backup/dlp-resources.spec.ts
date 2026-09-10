import type {
  DataPatternResponse,
  DataProfileResponse,
  DictionaryResponse,
} from '@cdot65/prisma-airs-sdk';
import {
  backupDlpResources,
  compareEcho,
  type DlpResourcesBackup,
  type DlpTransferApi,
  planDlpResourcesRestore,
  restoreDlpResources,
} from '../../../src/backup/dlp-resources.js';

const srcDictionary = (): DictionaryResponse => ({
  id: 'src-dict-1',
  name: 'Keywords',
  type: 'custom',
  category: 'Financial',
  region_name: 'us',
  is_case_sensitive: false,
  keywords: ['beta', 'alpha'],
  dictionary_metadata: { original_file_name: 'kw.txt' },
});
const srcPattern = (): DataPatternResponse => ({
  id: 'src-pat-1',
  name: 'Custom Regex',
  type: 'custom',
  status: 'active',
  version: 2,
  tenant_id: 'tenant-src',
  detection_config: { technique: 'regex', supported_confidence_levels: ['high'] },
  matching_rules: { regexes: [{ regex: 'a+', weight: 10 }] },
});
const srcPredefined = (): DataPatternResponse => ({
  id: 'src-pat-pre',
  name: 'SSN',
  type: 'predefined',
  status: 'active',
  version: 3,
  detection_config: { technique: 'ml' },
});
const srcEdm = (): DataPatternResponse => ({
  id: 'src-pat-edm',
  name: 'EDM SSN',
  type: 'custom',
  status: 'active',
  version: 1,
  tenant_id: 'tenant-src',
  detection_config: { technique: 'edm' },
});
const srcProfile = (): DataProfileResponse => ({
  id: 'src-prof-1',
  name: 'Profile One',
  type: 'custom',
  profile_status: 'active',
  profile_type: 'advanced',
  tenant_id: 'tenant-src',
  detection_rules: [
    {
      rule_type: 'expression_tree',
      expression_tree: {
        operator_type: 'or',
        sub_expressions: [
          {
            rule_item: {
              detection_technique: 'regex',
              id: 'src-pat-1',
              name: 'Custom Regex',
              version: 2,
              confidence_level: 'high',
            },
          },
          { rule_item: { detection_technique: 'dictionary', id: 'src-dict-1', name: 'Keywords' } },
          { rule_item: { detection_technique: 'ml', id: 'src-pat-pre', name: 'SSN', version: 3 } },
        ],
      },
    },
  ],
});
const destPredefined = (): DataPatternResponse => ({
  id: 'dest-pat-pre',
  name: 'SSN',
  type: 'predefined',
  status: 'active',
  version: 7,
  tenant_id: 'tenant-dest',
  detection_config: { technique: 'ml' },
});

interface Seed {
  dictionaries?: DictionaryResponse[];
  patterns?: DataPatternResponse[];
  profiles?: DataProfileResponse[];
}

function memoryApi(seed: Seed = {}) {
  const state = {
    dictionaries: structuredClone(seed.dictionaries ?? []),
    patterns: structuredClone(seed.patterns ?? []),
    profiles: structuredClone(seed.profiles ?? []),
  };
  let next = 0;
  const api = {
    dictionaries: {
      list: vi.fn(async () => ({ content: structuredClone(state.dictionaries), last: true })),
      create: vi.fn(async ({ metadata, file }) => {
        const { original_file_name, ...rest } = metadata;
        const record: DictionaryResponse = {
          ...rest,
          id: `dest-dict-${next++}`,
          type: 'custom',
          keywords: String(file).split('\n').filter(Boolean),
          dictionary_metadata: { original_file_name },
        };
        state.dictionaries.push(record);
        return structuredClone(record);
      }),
      get: vi.fn(async (id) => {
        const record = state.dictionaries.find((d) => d.id === id);
        if (!record) throw new Error('missing dictionary');
        return structuredClone(record);
      }),
    },
    patterns: {
      list: vi.fn(async () => ({ content: structuredClone(state.patterns), last: true })),
      create: vi.fn(async (body) => {
        const record: DataPatternResponse = {
          ...structuredClone(body),
          id: `dest-pat-${next++}`,
          status: 'active',
          version: 9,
          tenant_id: 'tenant-dest',
        };
        state.patterns.push(record);
        return structuredClone(record);
      }),
      get: vi.fn(async (id) => {
        const record = state.patterns.find((p) => p.id === id);
        if (!record) throw new Error('missing pattern');
        return structuredClone(record);
      }),
    },
    profiles: {
      list: vi.fn(async () => ({ content: structuredClone(state.profiles), last: true })),
      create: vi.fn(async (body) => {
        const record: DataProfileResponse = {
          ...structuredClone(body),
          id: `dest-prof-${next++}`,
          type: 'custom',
          profile_status: 'active',
          tenant_id: 'tenant-dest',
        };
        state.profiles.push(record);
        return structuredClone(record);
      }),
      get: vi.fn(async (id) => {
        const record = state.profiles.find((p) => p.id === id);
        if (!record) throw new Error('missing profile');
        return structuredClone(record);
      }),
    },
  } satisfies DlpTransferApi;
  return { api, state };
}

function sourceApi(extra: Seed = {}) {
  return memoryApi({
    dictionaries: [srcDictionary(), ...(extra.dictionaries ?? [])],
    patterns: [srcPattern(), srcPredefined(), ...(extra.patterns ?? [])],
    profiles: [srcProfile(), ...(extra.profiles ?? [])],
  });
}

async function sourceEnvelope(extra: Seed = {}): Promise<DlpResourcesBackup> {
  return (await backupDlpResources(sourceApi(extra).api, '100')).backup;
}

describe('backupDlpResources', () => {
  it('exports custom resources with dependency closure, sorted and complete', async () => {
    const { backup, skipped } = await backupDlpResources(sourceApi().api, '100');
    expect(skipped).toEqual([]);
    expect(backup.version).toBe(1);
    expect(backup.resourceType).toBe('dlp-resources');
    expect(backup.source).toEqual({ tsgId: '100' });
    expect(backup.dictionaries.map((d) => d.name)).toEqual(['Keywords']);
    expect(backup.patterns.map((p) => p.name)).toEqual(['Custom Regex', 'SSN']);
    expect(backup.profiles.map((p) => p.name)).toEqual(['Profile One']);
  });

  it('embeds referenced dependencies even when only profiles are selected', async () => {
    const { backup } = await backupDlpResources(sourceApi().api, '100', {
      resources: ['profiles'],
    });
    expect(backup.dictionaries.map((d) => d.name)).toEqual(['Keywords']);
    expect(backup.patterns.map((p) => p.name)).toEqual(['Custom Regex', 'SSN']);
    expect(backup.profiles).toHaveLength(1);
  });

  it('selects only the requested primary resources', async () => {
    const { backup } = await backupDlpResources(sourceApi().api, '100', {
      resources: ['patterns'],
    });
    expect(backup.patterns.map((p) => p.name)).toEqual(['Custom Regex']);
    expect(backup.dictionaries).toEqual([]);
    expect(backup.profiles).toEqual([]);
  });

  it('excludes retired patterns and predefined resources from primary selection', async () => {
    const retired: DataPatternResponse = {
      ...srcPattern(),
      id: 'dead',
      name: 'Dead',
      status: 'deleted',
    };
    const { backup } = await backupDlpResources(sourceApi({ patterns: [retired] }).api, '100', {
      resources: ['patterns', 'dictionaries'],
    });
    expect(backup.patterns.map((p) => p.name)).toEqual(['Custom Regex']);
  });

  it('fails on a profile with a multi-profile rule unless skipUnsupported', async () => {
    const multi: DataProfileResponse = {
      ...srcProfile(),
      id: 'src-prof-2',
      name: 'Multi',
      detection_rules: [{ rule_type: 'multi_profile', multi_profile: { data_profile_ids: [1] } }],
    };
    await expect(backupDlpResources(sourceApi({ profiles: [multi] }).api, '100')).rejects.toThrow(
      /Profiles cannot be exported: Multi \(Unsupported detection rule type: multi_profile\); exclude unsupported profiles explicitly with --skip-unsupported/,
    );
    const { backup, skipped } = await backupDlpResources(
      sourceApi({ profiles: [multi] }).api,
      '100',
      { skipUnsupported: true },
    );
    expect(skipped).toEqual([
      { profile: 'Multi', reason: expect.stringContaining('multi_profile') },
    ]);
    expect(backup.profiles.map((p) => p.name)).toEqual(['Profile One']);
  });

  it('reports every unsupported profile in one failure, not one at a time', async () => {
    const multi: DataProfileResponse = {
      ...srcProfile(),
      id: 'src-prof-m',
      name: 'Multi',
      detection_rules: [{ rule_type: 'multi_profile', multi_profile: { data_profile_ids: [1] } }],
    };
    const ghost: DataProfileResponse = {
      ...srcProfile(),
      id: 'src-prof-g',
      name: 'Ghost',
      detection_rules: [
        {
          rule_type: 'expression_tree',
          expression_tree: { rule_item: { detection_technique: 'regex', id: 'nope' } },
        },
      ],
    };
    const seed = { profiles: [multi, ghost] };
    await expect(backupDlpResources(sourceApi(seed).api, '100')).rejects.toThrow(
      /Multi \(Unsupported detection rule type: multi_profile\); Ghost \(references an unknown data pattern: nope\)/,
    );
    const { skipped } = await backupDlpResources(sourceApi(seed).api, '100', {
      skipUnsupported: true,
    });
    expect(skipped.map((item) => item.profile).sort()).toEqual(['Ghost', 'Multi']);
  });

  it('fails on a direct EDM dataset reference unless skipUnsupported', async () => {
    const direct: DataProfileResponse = {
      ...srcProfile(),
      id: 'src-prof-3',
      name: 'Direct EDM',
      detection_rules: [
        {
          rule_type: 'expression_tree',
          expression_tree: {
            rule_item: { detection_technique: 'edm', edm_dataset_id: 'ds-1' },
          },
        },
      ],
    };
    await expect(backupDlpResources(sourceApi({ profiles: [direct] }).api, '100')).rejects.toThrow(
      /EDM dataset directly/,
    );
  });

  it('fails when a profile references an unknown resource id', async () => {
    const ghost: DataProfileResponse = {
      ...srcProfile(),
      id: 'src-prof-4',
      name: 'Ghost',
      detection_rules: [
        {
          rule_type: 'expression_tree',
          expression_tree: { rule_item: { detection_technique: 'regex', id: 'nope' } },
        },
      ],
    };
    await expect(backupDlpResources(sourceApi({ profiles: [ghost] }).api, '100')).rejects.toThrow(
      /unknown data pattern: nope/,
    );
  });

  it('fails when a custom dictionary comes back without keywords', async () => {
    const bare = { ...srcDictionary(), keywords: undefined };
    const { api } = memoryApi({ dictionaries: [bare] });
    await expect(backupDlpResources(api, '100', { resources: ['dictionaries'] })).rejects.toThrow(
      /keywords were not returned/,
    );
  });

  it('fails on duplicate custom names', async () => {
    const twin = { ...srcPattern(), id: 'src-pat-9' };
    await expect(
      backupDlpResources(sourceApi({ patterns: [twin] }).api, '100', { resources: ['patterns'] }),
    ).rejects.toThrow(/duplicate data pattern names/);
  });

  it('rejects an unknown resource kind and an empty inventory', async () => {
    await expect(
      backupDlpResources(sourceApi().api, '100', { resources: ['nope' as never] }),
    ).rejects.toThrow(/resources must name/);
    await expect(backupDlpResources(memoryApi().api, '100')).rejects.toThrow(
      /No matching DLP resources/,
    );
    await expect(backupDlpResources(sourceApi().api, '')).rejects.toThrow(/TSG ID is required/);
  });

  it('refuses inventories that do not advance or exceed limits', async () => {
    const stuck = memoryApi().api;
    stuck.patterns.list = vi.fn(async () => ({ content: [], last: false })) as never;
    await expect(backupDlpResources(stuck, '100')).rejects.toThrow(/did not advance/);

    const big = memoryApi().api;
    big.dictionaries.list = vi.fn(async () => ({
      content: Array.from({ length: 5001 }, (_, i) => ({ id: `d${i}`, name: `d${i}` })),
      last: false,
    })) as never;
    await expect(backupDlpResources(big, '100')).rejects.toThrow(/safety limit/);

    const endless = memoryApi().api;
    endless.patterns.list = vi.fn(async () => ({
      content: [{ id: 'p', name: 'p' }],
      last: false,
    })) as never;
    await expect(backupDlpResources(endless, '100', { maxPages: 2 })).rejects.toThrow(
      /increase --max-pages/,
    );
    await expect(backupDlpResources(memoryApi().api, '100', { maxPages: 0 })).rejects.toThrow(
      /maxPages/,
    );
  });

  it('refuses mixed tenant identities in the inventory', async () => {
    const foreign = { ...srcPattern(), id: 'src-pat-8', name: 'Other', tenant_id: 'tenant-b' };
    await expect(backupDlpResources(sourceApi({ patterns: [foreign] }).api, '100')).rejects.toThrow(
      /mixed DLP tenant identities/,
    );
  });
});

describe('planDlpResourcesRestore', () => {
  it('plans a cross-tenant restore: create, resolve and stage order', async () => {
    const envelope = await sourceEnvelope();
    const { api } = memoryApi({ patterns: [destPredefined()] });
    const plan = await planDlpResourcesRestore(api, envelope, '200');
    expect(plan.dictionaries).toEqual([
      expect.objectContaining({ name: 'Keywords', action: 'create' }),
    ]);
    expect(plan.patterns).toEqual([
      expect.objectContaining({ name: 'Custom Regex', action: 'create' }),
      expect.objectContaining({ name: 'SSN', action: 'resolve' }),
    ]);
    expect(plan.profiles).toEqual([
      expect.objectContaining({ name: 'Profile One', action: 'create' }),
    ]);
  });

  it('fails when a referenced predefined pattern is missing, naming the mapping remedy', async () => {
    const envelope = await sourceEnvelope();
    await expect(planDlpResourcesRestore(memoryApi().api, envelope, '200')).rejects.toThrow(
      /Missing predefined destination data pattern: SSN; bind an equivalent destination pattern with --pattern-map "SSN=<destination-name>"/,
    );
  });

  it('binds a predefined reference through --pattern-map when catalogs differ', async () => {
    const envelope = await sourceEnvelope();
    const renamed: DataPatternResponse = {
      ...destPredefined(),
      id: 'dest-ssn',
      name: 'Social Security Numbers',
      version: 11,
    };
    const { api } = memoryApi({ patterns: [renamed] });
    const plan = await planDlpResourcesRestore(api, envelope, '200', {
      patternMap: { SSN: 'Social Security Numbers' },
    });
    expect(plan.patterns).toContainEqual(
      expect.objectContaining({ name: 'Social Security Numbers', action: 'map' }),
    );
    const result = await restoreDlpResources(api, plan);
    expect(result.complete).toBe(true);
    expect(result.patterns).toContainEqual({
      name: 'Social Security Numbers',
      action: 'mapped',
      id: 'dest-ssn',
    });
    const profileBody = api.profiles.create.mock.calls[0][0];
    const leaves = (
      profileBody.detection_rules?.[0] as {
        expression_tree?: { sub_expressions?: Array<{ rule_item?: Record<string, unknown> }> };
      }
    ).expression_tree?.sub_expressions?.map((node) => node.rule_item);
    expect(leaves?.[2]).toEqual(
      expect.objectContaining({ id: 'dest-ssn', name: 'Social Security Numbers', version: 11 }),
    );
  });

  it('requires --pattern-map for tenant-bound techniques and honors the mapping', async () => {
    const envelope = await sourceEnvelope({ patterns: [srcEdm()] });
    const destEdm: DataPatternResponse = {
      id: 'dest-edm',
      name: 'Dest EDM',
      type: 'custom',
      status: 'active',
      version: 4,
      tenant_id: 'tenant-dest',
      detection_config: { technique: 'edm' },
    };
    const { api } = memoryApi({ patterns: [destPredefined(), destEdm] });
    await expect(planDlpResourcesRestore(api, envelope, '200')).rejects.toThrow(
      /tenant-bound detection technique: EDM SSN/,
    );
    const plan = await planDlpResourcesRestore(api, envelope, '200', {
      patternMap: { 'EDM SSN': 'Dest EDM' },
    });
    expect(plan.patterns).toContainEqual(
      expect.objectContaining({ name: 'Dest EDM', action: 'map' }),
    );
    await expect(
      planDlpResourcesRestore(api, envelope, '200', { patternMap: { Ghost: 'Dest EDM' } }),
    ).rejects.toThrow(/absent from the backup: Ghost/);
    await expect(
      planDlpResourcesRestore(api, envelope, '200', { patternMap: { 'EDM SSN': 'Nope' } }),
    ).rejects.toThrow(/Missing mapped destination data pattern: Nope/);
  });

  it('refuses unknown cross-tenant references in pattern bodies', async () => {
    const envelope = await sourceEnvelope();
    const patterns = structuredClone(envelope.patterns);
    const custom = patterns.find((p) => p.name === 'Custom Regex');
    if (!custom) throw new Error('fixture');
    custom.matching_rules = { ...custom.matching_rules, linked_id: 'foreign' };
    const { api } = memoryApi({ patterns: [destPredefined()] });
    await expect(planDlpResourcesRestore(api, { ...envelope, patterns }, '200')).rejects.toThrow(
      /Unsupported cross-tenant reference in pattern Custom Regex/,
    );
  });

  it('fails when a profile references a resource absent from the backup', async () => {
    const envelope = await sourceEnvelope();
    const profiles = structuredClone(envelope.profiles);
    profiles[0].detection_rules = [
      {
        rule_type: 'expression_tree',
        expression_tree: { rule_item: { detection_technique: 'regex', id: 'ghost' } },
      },
    ];
    const { api } = memoryApi({ patterns: [destPredefined()] });
    await expect(planDlpResourcesRestore(api, { ...envelope, profiles }, '200')).rejects.toThrow(
      /references a resource absent from the backup: Profile One/,
    );
  });

  it('applies profile conflict policy: error, skip, and verify with missing dependencies', async () => {
    const envelope = await sourceEnvelope();
    const conflicting: DataProfileResponse = {
      ...srcProfile(),
      id: 'dest-prof-9',
      tenant_id: 'tenant-dest',
    };
    const { api } = memoryApi({ patterns: [destPredefined()], profiles: [conflicting] });
    await expect(planDlpResourcesRestore(api, envelope, '200')).rejects.toThrow(
      /already exists: Profile One/,
    );
    const plan = await planDlpResourcesRestore(api, envelope, '200', { onConflict: 'skip' });
    expect(plan.profiles).toEqual([expect.objectContaining({ action: 'skip' })]);
    await expect(
      planDlpResourcesRestore(api, envelope, '200', { onConflict: 'verify' }),
    ).rejects.toThrow(/cannot be verified without its destination dependencies/);
  });

  it('refuses a differing destination dictionary and suggests a prefix', async () => {
    const envelope = await sourceEnvelope();
    const different: DictionaryResponse = { ...srcDictionary(), id: 'dest-d', keywords: ['other'] };
    const { api } = memoryApi({ patterns: [destPredefined()], dictionaries: [different] });
    await expect(planDlpResourcesRestore(api, envelope, '200')).rejects.toThrow(
      /Destination dictionary differs: Keywords; use --name-prefix/,
    );
  });

  it('refuses a differing destination pattern and suggests a prefix', async () => {
    const envelope = await sourceEnvelope();
    const different: DataPatternResponse = {
      ...srcPattern(),
      id: 'dest-x',
      tenant_id: 'tenant-dest',
      matching_rules: { regexes: [{ regex: 'b+', weight: 2 }] },
    };
    const { api } = memoryApi({ patterns: [destPredefined(), different] });
    await expect(planDlpResourcesRestore(api, envelope, '200')).rejects.toThrow(
      /Destination data pattern differs: Custom Regex; use --name-prefix/,
    );
  });

  it('plans a create when the destination name collides only with a retired pattern', async () => {
    const envelope = await sourceEnvelope();
    const archived: DataPatternResponse = {
      ...srcPattern(),
      id: 'dead-1',
      status: 'deleted',
      tenant_id: 'tenant-dest',
    };
    const { api } = memoryApi({ patterns: [destPredefined(), archived] });
    const plan = await planDlpResourcesRestore(api, envelope, '200');
    expect(plan.patterns).toContainEqual(
      expect.objectContaining({ name: 'Custom Regex', action: 'create' }),
    );
    const result = await restoreDlpResources(api, plan);
    expect(result.complete).toBe(true);
  });

  it('validates options and envelope shape', async () => {
    const envelope = await sourceEnvelope();
    const { api } = memoryApi({ patterns: [destPredefined()] });
    await expect(
      planDlpResourcesRestore(api, envelope, '200', { namePrefix: 'bad' }),
    ).rejects.toThrow(/Invalid --name-prefix/);
    await expect(
      planDlpResourcesRestore(api, envelope, '200', { onConflict: 'update' as never }),
    ).rejects.toThrow(/Invalid conflict policy/);
    await expect(planDlpResourcesRestore(api, envelope, '')).rejects.toThrow(
      /Destination TSG ID is required/,
    );
    await expect(planDlpResourcesRestore(api, { nope: true }, '200')).rejects.toThrow();
  });

  it('checks tenant identity in both directions', async () => {
    const envelope = await sourceEnvelope();
    const sameTenant = memoryApi({
      patterns: [{ ...destPredefined(), tenant_id: 'tenant-src' }],
    });
    await expect(planDlpResourcesRestore(sameTenant.api, envelope, '200')).rejects.toThrow(
      /report the source DLP tenant/,
    );
    const otherTenant = memoryApi({ patterns: [destPredefined()] });
    await expect(planDlpResourcesRestore(otherTenant.api, envelope, '100')).rejects.toThrow(
      /does not match the backup source/,
    );
  });
});

describe('restoreDlpResources', () => {
  it('restores staged resources with remapped identities and verifies read-back', async () => {
    const envelope = await sourceEnvelope();
    const { api } = memoryApi({ patterns: [destPredefined()] });
    const plan = await planDlpResourcesRestore(api, envelope, '200', { namePrefix: 'mig-' });
    const result = await restoreDlpResources(api, plan);
    expect(result.error).toBeUndefined();
    expect(result.complete).toBe(true);
    expect(result.dictionaries).toEqual([
      { name: 'mig-Keywords', action: 'created', id: expect.any(String) },
    ]);
    expect(result.patterns).toEqual([
      { name: 'mig-Custom Regex', action: 'created', id: expect.any(String) },
      { name: 'SSN', action: 'resolved', id: 'dest-pat-pre' },
    ]);
    expect(result.profiles).toEqual([
      { name: 'mig-Profile One', action: 'created', id: expect.any(String) },
    ]);
    expect(api.dictionaries.create).toHaveBeenCalledWith({
      metadata: expect.objectContaining({ name: 'mig-Keywords', original_file_name: 'kw.txt' }),
      file: 'alpha\nbeta\n',
      includeKeywords: true,
    });
    const profileBody = api.profiles.create.mock.calls[0][0];
    const leaves = (
      profileBody.detection_rules?.[0] as {
        expression_tree?: { sub_expressions?: Array<{ rule_item?: Record<string, unknown> }> };
      }
    ).expression_tree?.sub_expressions?.map((node) => node.rule_item);
    expect(leaves).toEqual([
      expect.objectContaining({ name: 'mig-Custom Regex', version: 9 }),
      expect.objectContaining({ name: 'mig-Keywords' }),
      expect.objectContaining({ id: 'dest-pat-pre', name: 'SSN', version: 7 }),
    ]);
    expect(leaves?.[0]?.id).toBe(result.patterns[0].id);
    expect(leaves?.[1]?.id).toBe(result.dictionaries[0].id);
  });

  it('is resumable: a second pass reuses, resolves and verifies without writes', async () => {
    const envelope = await sourceEnvelope();
    const { api } = memoryApi({ patterns: [destPredefined()] });
    const first = await restoreDlpResources(
      api,
      await planDlpResourcesRestore(api, envelope, '200'),
    );
    expect(first.complete).toBe(true);
    const plan = await planDlpResourcesRestore(api, envelope, '200', { onConflict: 'verify' });
    const writesBefore =
      api.dictionaries.create.mock.calls.length + api.patterns.create.mock.calls.length;
    const result = await restoreDlpResources(api, plan);
    expect(result.complete).toBe(true);
    expect(result.dictionaries).toEqual([expect.objectContaining({ action: 'reused' })]);
    expect(result.patterns.map((p) => p.action)).toEqual(['reused', 'resolved']);
    expect(result.profiles).toEqual([expect.objectContaining({ action: 'verified' })]);
    expect(api.dictionaries.create.mock.calls.length + api.patterns.create.mock.calls.length).toBe(
      writesBefore,
    );
    expect(api.profiles.create).toHaveBeenCalledTimes(1);
  });

  it('skips conflicting profiles under --on-conflict skip', async () => {
    const envelope = await sourceEnvelope();
    const { api } = memoryApi({ patterns: [destPredefined()] });
    await restoreDlpResources(api, await planDlpResourcesRestore(api, envelope, '200'));
    const plan = await planDlpResourcesRestore(api, envelope, '200', { onConflict: 'skip' });
    const result = await restoreDlpResources(api, plan);
    expect(result.complete).toBe(true);
    expect(result.profiles).toEqual([expect.objectContaining({ action: 'skipped' })]);
    expect(api.profiles.create).toHaveBeenCalledTimes(1);
  });

  it('stops before any write when the destination changed after planning', async () => {
    const envelope = await sourceEnvelope();
    const { api, state } = memoryApi({ patterns: [destPredefined()] });
    const plan = await planDlpResourcesRestore(api, envelope, '200');
    state.profiles.push({ ...srcProfile(), id: 'raced', tenant_id: 'tenant-dest' });
    const result = await restoreDlpResources(api, plan);
    expect(result.complete).toBe(false);
    expect(result.error).toMatch(/data profile changed after planning: Profile One/);
    expect(api.dictionaries.create).not.toHaveBeenCalled();
    expect(api.patterns.create).not.toHaveBeenCalled();
    expect(api.profiles.create).not.toHaveBeenCalled();
  });

  it('reports a failed read-back verification and the writes that completed', async () => {
    const envelope = await sourceEnvelope();
    const { api, state } = memoryApi({ patterns: [destPredefined()] });
    const plan = await planDlpResourcesRestore(api, envelope, '200');
    api.patterns.get.mockImplementation(async (id: string) => {
      const record = state.patterns.find((p) => p.id === id);
      if (!record) throw new Error('missing pattern');
      return { ...structuredClone(record), matching_rules: { regexes: [] } };
    });
    const result = await restoreDlpResources(api, plan);
    expect(result.complete).toBe(false);
    expect(result.error).toMatch(/Restored data pattern did not verify: Custom Regex/);
    expect(result.dictionaries).toEqual([expect.objectContaining({ action: 'created' })]);
    expect(api.profiles.create).not.toHaveBeenCalled();
  });

  it('fails dictionary verification when keywords do not round-trip', async () => {
    const envelope = await sourceEnvelope();
    const { api } = memoryApi({ patterns: [destPredefined()] });
    const plan = await planDlpResourcesRestore(api, envelope, '200');
    api.dictionaries.get.mockImplementation(async () => ({
      ...srcDictionary(),
      id: 'dest-dict-x',
      keywords: ['tampered'],
    }));
    const result = await restoreDlpResources(api, plan);
    expect(result.complete).toBe(false);
    expect(result.error).toMatch(/Restored dictionary did not verify: Keywords/);
  });

  it('publishes only sanitized error messages', async () => {
    const envelope = await sourceEnvelope();
    const withStatus = memoryApi({ patterns: [destPredefined()] });
    const planA = await planDlpResourcesRestore(withStatus.api, envelope, '200');
    withStatus.api.dictionaries.create.mockRejectedValue(
      Object.assign(new Error('response body with keywords'), { statusCode: 500 }),
    );
    const resultA = await restoreDlpResources(withStatus.api, planA);
    expect(resultA.error).toBe(
      'API request failed (HTTP 500); verify destination state before retrying',
    );

    const opaque = memoryApi({ patterns: [destPredefined()] });
    const planB = await planDlpResourcesRestore(opaque.api, envelope, '200');
    opaque.api.dictionaries.create.mockRejectedValue(new Error('ECONNREFUSED secret-host'));
    const resultB = await restoreDlpResources(opaque.api, planB);
    expect(resultB.error).toBe('Restore stopped; verify destination state before retrying');
  });
});

const srcPreDict = (): DictionaryResponse => ({
  id: 'src-dict-pre',
  name: 'Legal Terms',
  type: 'predefined',
  category: 'Legal',
  region_name: 'us',
});
const preDictProfile = (): DataProfileResponse => ({
  id: 'src-prof-legal',
  name: 'Legal Profile',
  type: 'custom',
  profile_status: 'active',
  profile_type: 'advanced',
  tenant_id: 'tenant-src',
  detection_rules: [
    {
      rule_type: 'expression_tree',
      expression_tree: {
        rule_item: { detection_technique: 'dictionary', id: 'src-dict-pre', name: 'Legal Terms' },
      },
    },
  ],
});

describe('version and lifecycle fidelity', () => {
  it('fails export when a leaf pins a pattern version the catalog no longer holds', async () => {
    const moved = { ...srcPattern(), version: 5 };
    const { api } = memoryApi({
      dictionaries: [srcDictionary()],
      patterns: [moved, srcPredefined()],
      profiles: [srcProfile()],
    });
    await expect(backupDlpResources(api, '100')).rejects.toThrow(
      /references data pattern version 2, but the catalog holds version 5/,
    );
    const { skipped } = await backupDlpResources(api, '100', { skipUnsupported: true });
    expect(skipped).toEqual([
      { profile: 'Profile One', reason: expect.stringContaining('catalog holds version 5') },
    ]);
  });

  it('fails export when a leaf pins a dictionary version', async () => {
    const pinned: DataProfileResponse = {
      ...preDictProfile(),
      detection_rules: [
        {
          rule_type: 'expression_tree',
          expression_tree: {
            rule_item: {
              detection_technique: 'dictionary',
              id: 'src-dict-1',
              name: 'Keywords',
              version: 1,
            },
          },
        },
      ],
    };
    await expect(backupDlpResources(sourceApi({ profiles: [pinned] }).api, '100')).rejects.toThrow(
      /version-pinned dictionary/,
    );
  });

  it('fails verification when a created record reads back in a retired state', async () => {
    const envelope = await sourceEnvelope();
    const { api, state } = memoryApi({ patterns: [destPredefined()] });
    const plan = await planDlpResourcesRestore(api, envelope, '200');
    api.patterns.get.mockImplementation(async (id: string) => {
      const record = state.patterns.find((p) => p.id === id);
      if (!record) throw new Error('missing pattern');
      return { ...structuredClone(record), status: 'disabled' as const };
    });
    const result = await restoreDlpResources(api, plan);
    expect(result.complete).toBe(false);
    expect(result.error).toMatch(/Restored data pattern did not verify: Custom Regex/);

    const again = memoryApi({ patterns: [destPredefined()] });
    const planB = await planDlpResourcesRestore(again.api, envelope, '200');
    again.api.profiles.get.mockImplementation(async (id: string) => {
      const record = again.state.profiles.find((p) => p.id === id);
      if (!record) throw new Error('missing profile');
      return { ...structuredClone(record), profile_status: 'disabled' as const };
    });
    const resultB = await restoreDlpResources(again.api, planB);
    expect(resultB.complete).toBe(false);
    expect(resultB.error).toMatch(/Restored data profile did not verify: Profile One/);
  });
});

describe('predefined dictionary resolution', () => {
  const legalSeed: Seed = { dictionaries: [srcPreDict()], profiles: [preDictProfile()] };

  it('embeds the reference and resolves it in the destination', async () => {
    const envelope = await sourceEnvelope(legalSeed);
    expect(envelope.dictionaries.map((d) => d.name)).toEqual(['Keywords', 'Legal Terms']);
    const destPreDict: DictionaryResponse = {
      id: 'dest-dict-pre',
      name: 'Legal Terms',
      type: 'predefined',
      category: 'Legal',
      region_name: 'us',
    };
    const { api } = memoryApi({ patterns: [destPredefined()], dictionaries: [destPreDict] });
    const plan = await planDlpResourcesRestore(api, envelope, '200');
    expect(plan.dictionaries).toContainEqual(
      expect.objectContaining({ name: 'Legal Terms', action: 'resolve' }),
    );
    const result = await restoreDlpResources(api, plan);
    expect(result.complete).toBe(true);
    expect(result.dictionaries).toContainEqual({
      name: 'Legal Terms',
      action: 'resolved',
      id: 'dest-dict-pre',
    });
    const legalBody = api.profiles.create.mock.calls
      .map(([body]) => body)
      .find((body) => body.name === 'Legal Profile');
    expect(
      (legalBody?.detection_rules?.[0] as { expression_tree?: { rule_item?: { id?: string } } })
        .expression_tree?.rule_item?.id,
    ).toBe('dest-dict-pre');
  });

  it('fails when the predefined dictionary is missing or not predefined in the destination', async () => {
    const envelope = await sourceEnvelope(legalSeed);
    await expect(
      planDlpResourcesRestore(memoryApi({ patterns: [destPredefined()] }).api, envelope, '200'),
    ).rejects.toThrow(/Missing predefined destination dictionary: Legal Terms/);
    const imposter: DictionaryResponse = {
      ...srcPreDict(),
      id: 'dest-custom',
      type: 'custom',
      keywords: ['x'],
    };
    await expect(
      planDlpResourcesRestore(
        memoryApi({ patterns: [destPredefined()], dictionaries: [imposter] }).api,
        envelope,
        '200',
      ),
    ).rejects.toThrow(/Missing predefined destination dictionary: Legal Terms/);
  });
});

describe('verify-mismatch detection', () => {
  it('refuses --on-conflict verify when the destination diverges from the plan', async () => {
    const envelope = await sourceEnvelope();
    const { api } = memoryApi({ patterns: [destPredefined()] });
    const first = await restoreDlpResources(
      api,
      await planDlpResourcesRestore(api, envelope, '200'),
    );
    expect(first.complete).toBe(true);
    const drifted = structuredClone(envelope);
    drifted.profiles[0].description = 'changed detection intent';
    await expect(
      planDlpResourcesRestore(api, drifted, '200', { onConflict: 'verify' }),
    ).rejects.toThrow(/does not match restore plan: Profile One/);
  });
});

describe('compareEcho', () => {
  it('unifies null and absent, reports server additions, ignores top-level metadata', () => {
    const result = compareEcho(
      { a: 1, b: null, nested: { keep: 'x' } },
      { a: 1, nested: { keep: 'x', added: 'y' }, id: 'server', audit_metadata: {} },
    );
    expect(result.matches).toBe(true);
    expect(result.serverAdded).toEqual(['body.nested.added']);
  });

  it('fails on changed values, missing fields, unknown top-level additions and array drift', () => {
    expect(compareEcho({ a: 1 }, { a: 2 }).matches).toBe(false);
    expect(compareEcho({ a: 1 }, {}).matches).toBe(false);
    expect(compareEcho({ list: [1, 2] }, { list: [1] }).matches).toBe(false);
    expect(compareEcho({ list: [1] }, { list: [2] }).differences).toEqual(['body.list[0]']);
    const stranger = compareEcho({ a: 1 }, { a: 1, surprise: true });
    expect(stranger.matches).toBe(true);
    expect(stranger.serverAdded).toEqual(['body.surprise']);
  });
});
