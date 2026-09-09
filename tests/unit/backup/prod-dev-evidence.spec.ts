import { validateEvidence } from '../../../scripts/validate-prod-dev-evidence.mjs';

// Entirely synthetic evidence: no customer identifiers, audit identities or captured files.
function fixture() {
  const files: Record<string, string> = {};
  const put = (name: string, value: unknown) => {
    files[name] = JSON.stringify(value);
  };
  const names = ['migration-basic-dlp', 'migration-custom-dlp', 'migration-topics-only'];
  const topicNames = ['migration-financial-advice', 'migration-legal-advice'];
  const tenants = [
    { name: 'prod', tsgId: '100', active: true },
    { name: 'dev', tsgId: '200', active: false },
  ];
  put('tenants.json', tenants);
  for (const file of ['dev-selected.json', 'final-tenants.json'])
    put(
      file,
      tenants.map((t) => ({ ...t, active: t.name === 'dev' })),
    );
  for (const prefix of ['prod-before', 'dev-before', 'dev-pre-restore', 'dev-after-preview'])
    for (const kind of ['profiles', 'topics']) put(`${prefix}-${kind}.json`, []);
  for (const [prefix, tsg] of [
    ['prod', '100'],
    ['dev', '200'],
  ]) {
    const pattern = {
      id: `${prefix}-pattern`,
      name: 'dlp-test-pattern',
      status: 'active',
      version: 1,
      tenantId: `dlp-${tsg}`,
      detectionConfig: { technique: 'regex' },
      matchingRules: { regexes: [{ regex: 'AIRS-E2E-[0-9]{6}', weight: 1 }] },
    };
    const dlp = {
      id: '123',
      name: 'dlp-test',
      version: 1,
      tenantId: `dlp-${tsg}`,
      profileType: 'advanced',
      profileStatus: 'active',
      detectionRules: [
        {
          ruleType: 'expression_tree',
          expressionTree: {
            operatorType: 'or',
            subExpressions: [
              {
                ruleItem: {
                  id: pattern.id,
                  version: 1,
                  detectionTechnique: 'regex',
                  matchType: 'include',
                  confidenceLevel: 'high',
                  occurrenceOperatorType: 'any',
                  occurrenceCount: 1,
                },
              },
            ],
          },
        },
      ],
    };
    put(`${prefix}-pattern.json`, pattern);
    put(`${prefix}-dlp-test.json`, dlp);
    const topics = topicNames.map((topicName) => ({
      topicName,
      topicId: `${prefix}-${topicName}`,
      revision: 1,
      description: `Synthetic ${topicName}`,
      examples: ['Synthetic example'],
    }));
    const profiles = names.map((profileName) => ({
      profileName,
      profileId: `${prefix}-${profileName}`,
      revision: 1,
      active: true,
      policy: {
        'ai-security-profiles': [
          {
            'model-type': 'default',
            'model-configuration': {
              'data-protection': {
                'data-leak-detection': {
                  action: profileName === 'migration-topics-only' ? '' : 'block',
                  'mask-data-inline': false,
                  member:
                    profileName === 'migration-topics-only'
                      ? null
                      : [
                          profileName === 'migration-custom-dlp'
                            ? { text: 'dlp-test', id: dlp.id, version: '1' }
                            : { text: 'sensitive content', id: '', version: '2' },
                        ],
                },
              },
              'model-protection':
                profileName !== 'migration-topics-only'
                  ? []
                  : [
                      {
                        name: 'topic-guardrails',
                        action: 'allow',
                        'topic-list': [
                          {
                            action: 'block',
                            topic: topics.map((t) => ({
                              topic_id: t.topicId,
                              topic_name: t.topicName,
                              revision: t.revision,
                              severity: 'medium',
                            })),
                          },
                        ],
                      },
                    ],
            },
          },
        ],
      },
    }));
    put(`${prefix}-profiles.json`, profiles);
    put(`${prefix}-topics.json`, topics);
    for (const p of profiles) put(`${prefix}-${p.profileName}.json`, p);
    put(`${prefix === 'prod' ? 'prod-after' : 'dev-after-verify'}-profiles.json`, profiles);
    put(`${prefix === 'prod' ? 'prod-after' : 'dev-after-verify'}-topics.json`, topics);
    put(`${prefix}-backup-summary.json`, [{ sourceTsgId: tsg, profiles: 3, topics: 2 }]);
    put(`${prefix}-runtime-backup.json`, {
      source: { tsgId: tsg },
      resourceType: 'runtime-security-profiles',
      version: 1,
      profiles: profiles.map((p) => ({
        profile_name: p.profileName,
        profile_id: p.profileId,
        revision: p.revision,
        active: p.active,
        tsg_id: tsg,
        policy: p.policy,
      })),
      topics: topics.map((t) => ({
        topic_name: t.topicName,
        topic_id: t.topicId,
        revision: t.revision,
        description: t.description,
        examples: t.examples,
      })),
    });
  }
  put('dev-preview.json', [
    {
      sourceTsgId: '100',
      destinationTsgId: '200',
      dryRun: true,
      profiles: names.map((name) => ({ name, action: 'create' })),
      topics: topicNames.map((name) => ({ name, action: 'create' })),
      dlpMappings: [{ source: 'dlp-test', destination: 'dlp-test' }],
      dlpFallbacks: [],
    },
  ]);
  for (const [file, profileAction, topicAction] of [
    ['dev-restore.json', 'created', 'created'],
    ['dev-verify.json', 'verified', 'reused'],
  ])
    put(file, [
      {
        sourceTsgId: '100',
        destinationTsgId: '200',
        complete: true,
        dlpFallbacks: [],
        serverDefaults: [],
        profiles: names.map((name) => ({ name, id: `dev-${name}`, action: profileAction })),
        topics: topicNames.map((name) => ({ name, id: `dev-${name}`, action: topicAction })),
      },
    ]);
  for (const name of [
    'prod-topic-financial.json',
    'prod-topic-legal.json',
    'prod-apply-financial.json',
    'prod-apply-legal.json',
  ])
    files[name] = 'Synthetic human-readable acknowledgement';
  for (let i = 0; i < 5; i++) put(`synthetic-create-${i}.json`, { action: 'created' });
  for (const name of Object.keys(files)) {
    files[`${name}.exit-code.txt`] = '0\n';
    files[`${name}.command.txt`] = 'airs runtime synthetic-fixture\n';
    files[`${name}.stderr`] = '';
  }
  return files;
}

it('accepts complete synthetic migration with tenant-scoped DLP IDs', () => {
  const result = validateEvidence(fixture());
  expect(result.passed).toBe(true);
  expect(result.checks).toHaveLength(7);
  expect(result.capturedCommands).toBe(45);
});

it.each([
  ['failed command', 'dev-restore.json.exit-code.txt', () => '1'],
  ['malformed required JSON', 'dev-restore.json', () => '{'],
  ['wrong destination', 'dev-preview.json', (v: string) => v.replace('"200"', '"999"')],
  [
    'changed dry run inventory',
    'dev-after-preview-topics.json',
    () => '[{"topicName":"Unexpected"}]',
  ],
  [
    'wrong pattern reference',
    'dev-dlp-test.json',
    (v: string) => v.replace('dev-pattern', 'prod-pattern'),
  ],
  ['changed regex', 'dev-pattern.json', (v: string) => v.replace('AIRS-E2E-', 'CHANGED-')],
  [
    'changed profile policy',
    'dev-migration-basic-dlp.json',
    (v: string) => v.replace('block', 'allow'),
  ],
  [
    'changed verification inventory',
    'dev-after-verify-profiles.json',
    (v: string) => v.replace('"revision":1', '"revision":2'),
  ],
  [
    'changed source',
    'prod-after-topics.json',
    (v: string) => v.replace('Synthetic example', 'Changed'),
  ],
  [
    'backup no longer matches',
    'dev-runtime-backup.json',
    (v: string) => v.replace('block', 'allow'),
  ],
  [
    'Basic fallback used',
    'dev-restore.json',
    (v: string) => v.replace('"dlpFallbacks":[]', '"dlpFallbacks":[{}]'),
  ],
])('rejects %s', (_case, file, corrupt) => {
  const files = fixture();
  files[file] = corrupt(files[file]);
  expect(() => validateEvidence(files)).toThrow('Evidence check failed:');
});
