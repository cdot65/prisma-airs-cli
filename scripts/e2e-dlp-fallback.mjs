// Live destination acceptance. Creates/deletes only five uniquely named synthetic profiles.
// Existing tenant configs, active selection, and the user's backup remain unchanged.
import assert from 'node:assert/strict';
import { execFile } from 'node:child_process';
import { randomUUID } from 'node:crypto';
import { mkdtemp, readFile, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { promisify } from 'node:util';
import { ManagementClient } from '@cdot65/prisma-airs-sdk';

const exec = promisify(execFile);
const repo = fileURLToPath(new URL('..', import.meta.url));
const entry = process.env.AIRS_CLI_ENTRY ?? join(repo, 'dist/cli/index.js');
const env = Object.fromEntries(
  Object.entries(process.env).filter(
    ([key]) => !key.startsWith('PANW_') && key !== 'PRISMA_AIRS_CONFIG_PATH',
  ),
);
env.NO_COLOR = '1';
const tenantList = async () =>
  (await exec(process.execPath, [entry, 'tenant', 'list', '--output', 'json'], { env })).stdout;
const beforeRegistry = await tenantList();
const tenants = JSON.parse(beforeRegistry);
const sourceTenant = tenants.find(
  (t) => t.name === (process.env.AIRS_DLP_SOURCE_TENANT ?? 'aisecurity'),
);
const destination = tenants.find((t) => t.name === (process.env.AIRS_DLP_DEST_TENANT ?? 'cdot65'));
assert.ok(sourceTenant && destination, 'Register source/destination tenants before this test');
assert.notEqual(sourceTenant.tsgId, destination.tsgId);
const configBytes = await readFile(destination.configPath);
const config = JSON.parse(configBytes.toString());
assert.equal(config.mgmtTsgId, destination.tsgId);
env.PRISMA_AIRS_CONFIG_PATH = destination.configPath;
const client = new ManagementClient({
  clientId: config.mgmtClientId,
  clientSecret: config.mgmtClientSecret,
  tsgId: config.mgmtTsgId,
  apiEndpoint: config.mgmtEndpoint,
  tokenEndpoint: config.mgmtTokenEndpoint,
  numRetries: 0,
});
const directory = await mkdtemp(join(repo, 'artifacts/tenant-profiles-e2e-dlp-'));
const suffix = randomUUID().slice(0, 12);
const names = [
  `dlp-basic-e2e-${suffix}`,
  `dlp-fallback-e2e-${suffix}`,
  `dlp-severity-e2e-${suffix}`,
  `dlp-category-e2e-${suffix}`,
  `dlp-category-explicit-e2e-${suffix}`,
];
const owned = new Set();
const results = [];
const transcript = [];
const policy = (custom, categories = false) => ({
  'ai-security-profiles': [
    {
      'model-type': 'default',
      'model-configuration': {
        'data-protection': {
          'database-security': ['create', 'read', 'update', 'delete'].map((name) => ({
            name: `database-security-${name}`,
            action: 'block',
          })),
          'source-code-detection': { action: 'block' },
          'data-leak-detection': {
            member: [
              {
                text: custom ? 'Synthetic missing DLP' : 'sensitive content',
                id: custom ? 'synthetic-source-only' : '',
                version: '2',
              },
            ],
            action: 'block',
            'mask-data-inline': true,
          },
        },
        'app-protection': {
          'url-detected-action': 'block',
          'malicious-code-protection': { name: 'malicious-code', action: 'block' },
        },
        'model-protection': [
          { name: 'prompt-injection', action: 'block' },
          categories
            ? {
                name: 'toxic-content',
                action: '',
                'toxic-category-list': [
                  'violent-crime',
                  'hate',
                  'sexual-content',
                  'indiscriminate-weapon',
                  'self-harm',
                  'cybercrime',
                  'controlled-substance',
                  'other-nonviolent-crime',
                ].map((category) => ({ category, action: 'high:allow, moderate:allow' })),
              }
            : { name: 'toxic-content', action: 'high:block, moderate:block' },
        ],
        'agent-protection': [{ name: 'agent-security', action: 'block' }],
      },
    },
  ],
  'dlp-data-profiles': [],
});
function withSeverities(input, override) {
  const result = structuredClone(input);
  const model = result['ai-security-profiles'][0]['model-configuration'];
  model['data-protection']['database-security'].forEach((item, i) => {
    item.severity = override ?? ['medium', 'low', 'medium', 'high'][i];
  });
  model['data-protection']['source-code-detection'].severity = override ?? 'high';
  model['app-protection']['malicious-code-protection'].severity = override ?? 'high';
  model['app-protection']['url-detected-severity'] = override ?? 'low';
  model['model-protection'][0].severity = override ?? 'medium';
  const toxicity = model['model-protection'][1];
  for (const item of toxicity['toxic-category-list'] ?? [toxicity]) {
    item['severity-by-confidence'] = {
      high: override ?? 'medium',
      moderate: override ?? 'low',
    };
  }
  model['agent-protection'][0].severity = override ?? 'medium';
  return result;
}
async function cli(args) {
  const result = await exec(process.execPath, [entry, ...args], {
    cwd: directory,
    env,
    timeout: 90000,
    maxBuffer: 5 * 1024 * 1024,
  }).catch((error) => error);
  for (const secret of [
    config.mgmtClientSecret,
    config.airsApiKey,
    config.aiGwInferenceApiKey,
  ].filter(Boolean))
    assert.ok(
      !(result.stdout + result.stderr).includes(secret),
      'Credential appeared in CLI output',
    );
  transcript.push({ args, stdout: result.stdout, stderr: result.stderr });
  return JSON.parse(result.stdout)[0];
}
try {
  assert.ok(
    !(await client.profiles.listAll({ latest: true })).some((p) => names.includes(p.profile_name)),
    'Synthetic name collision',
  );
  const backup = {
    version: 1,
    resourceType: 'runtime-security-profiles',
    exportedAt: new Date().toISOString(),
    source: { tsgId: sourceTenant.tsgId },
    topics: [],
    profiles: names.map((name, i) => ({
      profile_name: name,
      profile_id: `synthetic-source-${i}`,
      tsg_id: sourceTenant.tsgId,
      revision: 1,
      active: true,
      policy:
        i === 2 || i === 4
          ? withSeverities(policy(false, i === 4), 'low')
          : policy(i === 1, i === 3),
    })),
  };
  const file = join(directory, 'synthetic.json');
  const bytes = Buffer.from(JSON.stringify(backup));
  await writeFile(file, bytes, { mode: 0o600, flag: 'wx' });
  const args = [
    'runtime',
    'profiles',
    'restore',
    file,
    '--on-missing-dlp',
    'basic',
    '--output',
    'json',
  ];
  const plan = await cli([...args, '--dry-run']);
  assert.equal(plan.dlpFallbacks.length, 1);
  assert.equal(plan.dlpFallbacks[0].profile, names[1]);
  results.push({
    check: 'Live destination preflight preserves Basic and plans one custom fallback',
    passed: true,
  });
  const restored = await cli([...args, '--force', '--expect-tsg', destination.tsgId]);
  for (const profile of restored.profiles) if (profile.id) owned.add(profile.id);
  if (!restored.complete) {
    const actual = (await client.profiles.listAll({ latest: true }))
      .filter((p) => names.includes(p.profile_name))
      .map((p) => ({ name: p.profile_name, policy: p.policy }));
    console.log(JSON.stringify({ restoreError: restored.error, syntheticPolicies: actual }));
  }
  assert.equal(restored.complete, true, 'Restore verification failed');
  const inventory = await client.profiles.listAll({ latest: true });
  for (const [index, name] of names.entries()) {
    const found = inventory.find((profile) => profile.profile_name === name);
    assert.ok(found?.profile_id);
    owned.add(found.profile_id);
    const expected = withSeverities(
      policy(false, index >= 3),
      index === 2 || index === 4 ? 'low' : undefined,
    );
    expected['ai-security-profiles'][0]['model-configuration']['app-protection'][
      'default-url-category'
    ] = { member: null };
    assert.deepEqual(found.policy, expected);
  }
  assert.deepEqual(await readFile(file), bytes);
  results.push({
    check:
      'Live API accepted five Basic policies including nested toxicity; read-back and masking verified',
    passed: true,
    profiles: names.length,
    fallbacks: restored.dlpFallbacks.length,
  });
  assert.equal(restored.serverDefaults.length, 5);
  assert.deepEqual(
    restored.serverDefaults.map((item) => item.fields.length).sort((a, b) => a - b),
    [1, 1, 11, 11, 18],
  );
  const resumed = await cli([
    ...args,
    '--on-conflict',
    'verify',
    '--force',
    '--expect-tsg',
    destination.tsgId,
  ]);
  assert.equal(resumed.complete, true);
  assert.ok(resumed.profiles.every((item) => item.action === 'verified'));
  const after = await client.profiles.listAll({ latest: true });
  for (const item of inventory.filter((p) => names.includes(p.profile_name)))
    assert.deepEqual(
      after.find((p) => p.profile_name === item.profile_name),
      item,
    );
  results.push({
    check:
      'Explicit severities preserved; verified rerun leaves IDs, revisions and policies unchanged',
    passed: true,
  });
} catch (error) {
  results.push({
    check: 'Live fallback acceptance',
    passed: false,
    errorType: error?.name,
    statusCode: error?.statusCode,
  });
  process.exitCode = 1;
} finally {
  try {
    for (const p of await client.profiles.listAll({ latest: true }))
      if (names.includes(p.profile_name) && p.profile_id) owned.add(p.profile_id);
    for (const id of owned) await client.profiles.delete(id);
    assert.ok(
      !(await client.profiles.listAll({ latest: true })).some((p) =>
        names.includes(p.profile_name),
      ),
    );
    results.push({
      check: 'Only owned synthetic profiles removed; absence verified',
      passed: true,
      removed: owned.size,
    });
  } catch (error) {
    results.push({
      check: 'Cleanup',
      passed: false,
      statusCode: error?.statusCode,
      ownedIds: [...owned],
    });
    process.exitCode = 1;
  }
  assert.deepEqual(await readFile(destination.configPath), configBytes);
  assert.equal(await tenantList(), beforeRegistry);
  results.push({ check: 'Tenant configuration and selection unchanged', passed: true });
  await writeFile(
    join(directory, 'results.json'),
    JSON.stringify({ at: new Date().toISOString(), results }, null, 2),
    { mode: 0o600, flag: 'wx' },
  );
  await writeFile(join(directory, 'cli-transcript.json'), JSON.stringify(transcript, null, 2), {
    mode: 0o600,
    flag: 'wx',
  });
  console.log(JSON.stringify({ directory, results }));
}
