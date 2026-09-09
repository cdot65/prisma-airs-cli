// Offline validation of the three-profile prod/dev documentation fixture. No API calls or extraction.
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { pathToFileURL } from 'node:url';

export function readArchive(archive) {
  const names = execFileSync('tar', ['-tzf', archive], {
    encoding: 'utf8',
    maxBuffer: 1_000_000,
    timeout: 10000,
  })
    .trim()
    .split('\n');
  assert.ok(names.length <= 500, 'Archive entry limit exceeded');
  assert.equal(new Set(names).size, names.length, 'Duplicate archive entries');
  const files = {};
  for (const name of names) {
    assert.ok(
      /^[\w./-]+$/.test(name) && !name.startsWith('/') && !name.split('/').includes('..'),
      'Unsafe archive entry',
    );
    if (name.endsWith('/') || name.split('/').some((part) => part.startsWith('._'))) continue;
    const parts = name.split('/');
    assert.equal(parts.length, 2, 'Expected one evidence directory');
    const leaf = parts[1];
    assert.ok(!Object.hasOwn(files, leaf), 'Duplicate evidence filename');
    files[leaf] = execFileSync('tar', ['-xOzf', archive, '--', name], {
      encoding: 'utf8',
      maxBuffer: 2_000_000,
      timeout: 10000,
    });
  }
  return files;
}

export function validateEvidence(files) {
  const checks = [];
  const check = (name, fn) => {
    try {
      fn();
    } catch {
      throw new Error(`Evidence check failed: ${name}`);
    }
    checks.push(name);
  };
  const json = (name) => JSON.parse(files[name]);
  const row = (name) => {
    const rows = json(name);
    assert.equal(rows.length, 1);
    return rows[0];
  };
  const ordered = (rows, key) =>
    [...rows].sort((a, b) => String(a[key]).localeCompare(String(b[key])));
  const profiles = ['migration-basic-dlp', 'migration-custom-dlp', 'migration-topics-only'];
  const topics = ['migration-financial-advice', 'migration-legal-advice'];
  const textAcknowledgements = [
    'prod-topic-financial.json',
    'prod-topic-legal.json',
    'prod-apply-financial.json',
    'prod-apply-legal.json',
  ];
  let receipts;
  let prodTsg;
  let devTsg;

  check('captured commands completed and expected structured evidence parses', () => {
    receipts = Object.keys(files).filter((name) => name.endsWith('.exit-code.txt'));
    assert.ok(receipts.length >= 45);
    for (const receipt of receipts) {
      const name = receipt.slice(0, -'.exit-code.txt'.length);
      assert.equal(files[receipt].trim(), '0');
      assert.ok(Object.hasOwn(files, name));
      assert.ok(Object.hasOwn(files, `${name}.stderr`));
      assert.match(files[`${name}.command.txt`], /^airs (runtime|tenant) /);
    }
    for (const name of Object.keys(files).filter(
      (n) => n.endsWith('.json') && !textAcknowledgements.includes(n),
    ))
      json(name);
    for (const name of textAcknowledgements) assert.ok(files[name]?.trim());
  });
  check('distinct registered tenants and final dev selection', () => {
    const tenants = json('tenants.json');
    prodTsg = tenants.find((t) => t.name === 'prod')?.tsgId;
    devTsg = tenants.find((t) => t.name === 'dev')?.tsgId;
    assert.match(prodTsg, /^\d+$/);
    assert.match(devTsg, /^\d+$/);
    assert.notEqual(prodTsg, devTsg);
    for (const file of ['dev-selected.json', 'final-tenants.json']) {
      const rows = json(file);
      assert.deepEqual(
        rows.filter((t) => t.active).map((t) => t.name),
        ['dev'],
      );
      assert.equal(rows.find((t) => t.name === 'dev')?.tsgId, devTsg);
      assert.equal(rows.find((t) => t.name === 'prod')?.tsgId, prodTsg);
      assert.deepEqual(
        rows.map(({ active, ...r }) => r),
        tenants.map(({ active, ...r }) => r),
      );
    }
  });
  check('empty starting inventories and unchanged dry-run inventories', () => {
    for (const prefix of ['prod-before', 'dev-before', 'dev-pre-restore', 'dev-after-preview']) {
      for (const kind of ['profiles', 'topics'])
        assert.deepEqual(json(`${prefix}-${kind}.json`), []);
    }
    const preview = row('dev-preview.json');
    assert.equal(preview.sourceTsgId, prodTsg);
    assert.equal(preview.destinationTsgId, devTsg);
    assert.equal(preview.dryRun, true);
    assert.deepEqual(
      preview.profiles,
      profiles.map((name) => ({ name, action: 'create' })),
    );
    assert.deepEqual(
      preview.topics,
      topics.map((name) => ({ name, action: 'create' })),
    );
    assert.deepEqual(preview.dlpMappings, [{ source: 'dlp-test', destination: 'dlp-test' }]);
    assert.deepEqual(preview.dlpFallbacks, []);
  });
  check('synthetic custom DLP rules equivalent after tenant-local pattern binding', () => {
    const normalize = (value, patternId) => {
      if (Array.isArray(value)) return value.map((v) => normalize(v, patternId));
      if (!value || typeof value !== 'object') return value;
      return Object.fromEntries(
        Object.entries(value).map(([k, v]) => [
          k,
          k === 'id' && v === patternId ? '<pattern>' : normalize(v, patternId),
        ]),
      );
    };
    for (const prefix of ['prod', 'dev']) {
      const p = json(`${prefix}-pattern.json`),
        d = json(`${prefix}-dlp-test.json`);
      assert.equal(p.name, 'dlp-test-pattern');
      assert.equal(p.status, 'active');
      assert.equal(p.detectionConfig.technique, 'regex');
      assert.deepEqual(p.matchingRules.regexes, [{ regex: 'AIRS-E2E-[0-9]{6}', weight: 1 }]);
      assert.equal(d.name, 'dlp-test');
      assert.equal(d.profileType, 'advanced');
      assert.equal(d.profileStatus, 'active');
      assert.equal(p.tenantId, d.tenantId);
      assert.ok(Number.isInteger(d.version));
      const leaf = d.detectionRules[0].expressionTree.subExpressions[0].ruleItem;
      assert.equal(leaf.id, p.id);
      assert.equal(leaf.version, p.version);
      assert.equal(leaf.detectionTechnique, 'regex');
      assert.equal(leaf.matchType, 'include');
      assert.equal(leaf.confidenceLevel, 'high');
      assert.equal(leaf.occurrenceOperatorType, 'any');
    }
    const p = json('prod-pattern.json'),
      d = json('dev-pattern.json');
    assert.notEqual(p.tenantId, d.tenantId);
    assert.notEqual(p.id, d.id);
    assert.deepEqual(p.detectionConfig, d.detectionConfig);
    assert.deepEqual(p.matchingRules, d.matchingRules);
    assert.deepEqual(
      normalize(json('prod-dlp-test.json').detectionRules, p.id),
      normalize(json('dev-dlp-test.json').detectionRules, d.id),
    );
  });
  check('three full policies preserved with destination DLP and topic references', () => {
    for (const prefix of ['prod', 'dev']) {
      const inventory = json(`${prefix}-profiles.json`),
        topicInventory = json(`${prefix}-topics.json`);
      assert.deepEqual(inventory.map((p) => p.profileName).sort(), profiles);
      assert.deepEqual(topicInventory.map((t) => t.topicName).sort(), topics);
      for (const name of profiles) {
        const p = json(`${prefix}-${name}.json`);
        assert.deepEqual(
          p,
          inventory.find((item) => item.profileName === name),
        );
        assert.equal(p.active, true);
      }
      const config = (name) =>
        json(`${prefix}-${name}.json`).policy['ai-security-profiles'][0]['model-configuration'];
      const custom = config('migration-custom-dlp')['data-protection']['data-leak-detection'];
      const dlp = json(`${prefix}-dlp-test.json`);
      assert.deepEqual(custom, {
        action: 'block',
        member: [{ text: 'dlp-test', id: String(dlp.id), version: String(dlp.version) }],
        'mask-data-inline': false,
      });
      assert.deepEqual(config('migration-basic-dlp')['data-protection']['data-leak-detection'], {
        action: 'block',
        member: [{ text: 'sensitive content', id: '', version: '2' }],
        'mask-data-inline': false,
      });
      const cfg = config('migration-topics-only');
      assert.deepEqual(cfg['data-protection']['data-leak-detection'], {
        action: '',
        member: null,
        'mask-data-inline': false,
      });
      const guard = cfg['model-protection'].find((v) => v.name === 'topic-guardrails');
      assert.equal(guard.action, 'allow');
      assert.equal(guard['topic-list'].length, 1);
      assert.equal(guard['topic-list'][0].action, 'block');
      const refs = guard['topic-list'][0].topic;
      assert.deepEqual(refs.map((t) => t.topic_name).sort(), topics);
      for (const ref of refs) {
        const topic = topicInventory.find((t) => t.topicName === ref.topic_name);
        assert.equal(ref.topic_id, topic.topicId);
        assert.equal(ref.revision, topic.revision);
      }
    }
    const sourceTopics = json('prod-topics.json'),
      destinationTopics = json('dev-topics.json');
    for (const topic of sourceTopics) {
      const target = destinationTopics.find((t) => t.topicName === topic.topicName);
      assert.notEqual(topic.topicId, target.topicId);
      assert.equal(topic.description, target.description);
      assert.deepEqual(topic.examples, target.examples);
    }
    for (const name of profiles) {
      const source = json(`prod-${name}.json`),
        destination = json(`dev-${name}.json`);
      assert.notEqual(source.profileId, destination.profileId);
      const expected = structuredClone(source.policy);
      const cfg = expected['ai-security-profiles'][0]['model-configuration'];
      if (name === 'migration-custom-dlp') {
        const dlp = json('dev-dlp-test.json');
        cfg['data-protection']['data-leak-detection'].member = [
          { text: dlp.name, id: String(dlp.id), version: String(dlp.version) },
        ];
      }
      if (name === 'migration-topics-only')
        for (const ref of cfg['model-protection'][0]['topic-list'][0].topic) {
          const target = destinationTopics.find((t) => t.topicName === ref.topic_name);
          ref.topic_id = target.topicId;
          ref.revision = target.revision;
        }
      assert.deepEqual(destination.policy, expected);
    }
  });
  check('restore completed and verification reused resources without revision changes', () => {
    for (const [file, profileAction, topicAction] of [
      ['dev-restore.json', 'created', 'created'],
      ['dev-verify.json', 'verified', 'reused'],
    ]) {
      const r = row(file);
      assert.equal(r.sourceTsgId, prodTsg);
      assert.equal(r.destinationTsgId, devTsg);
      assert.equal(r.complete, true);
      assert.deepEqual(r.dlpFallbacks, []);
      assert.deepEqual(r.serverDefaults, []);
      assert.deepEqual(r.profiles.map((p) => p.name).sort(), profiles);
      assert.deepEqual(r.topics.map((t) => t.name).sort(), topics);
      for (const p of r.profiles) {
        assert.equal(p.action, profileAction);
        assert.equal(p.id, json(`dev-${p.name}.json`).profileId);
      }
      for (const t of r.topics) {
        assert.equal(t.action, topicAction);
        assert.equal(t.id, json('dev-topics.json').find((v) => v.topicName === t.name).topicId);
      }
    }
    for (const [kind, key] of [
      ['profiles', 'profileId'],
      ['topics', 'topicId'],
    ]) {
      assert.deepEqual(
        ordered(json(`dev-${kind}.json`), key),
        ordered(json(`dev-after-verify-${kind}.json`), key),
      );
      assert.deepEqual(
        ordered(json(`prod-${kind}.json`), key),
        ordered(json(`prod-after-${kind}.json`), key),
      );
    }
  });
  check('source and destination backups match captured inventory policies and definitions', () => {
    for (const [prefix, tsg] of [
      ['prod', prodTsg],
      ['dev', devTsg],
    ]) {
      const summary = row(`${prefix}-backup-summary.json`),
        backup = json(`${prefix}-runtime-backup.json`);
      assert.equal(summary.sourceTsgId, tsg);
      assert.equal(summary.profiles, 3);
      assert.equal(summary.topics, 2);
      assert.equal(backup.source.tsgId, tsg);
      assert.equal(backup.resourceType, 'runtime-security-profiles');
      assert.equal(backup.version, 1);
      assert.deepEqual(backup.profiles.map((p) => p.profile_name).sort(), profiles);
      assert.deepEqual(backup.topics.map((t) => t.topic_name).sort(), topics);
      for (const p of backup.profiles) {
        const stored = json(`${prefix}-${p.profile_name}.json`);
        assert.equal(p.profile_id, stored.profileId);
        assert.equal(p.revision, stored.revision);
        assert.equal(p.active, stored.active);
        assert.equal(p.tsg_id, tsg);
        assert.deepEqual(p.policy, stored.policy);
      }
      for (const t of backup.topics) {
        const stored = json(`${prefix}-topics.json`).find((v) => v.topicName === t.topic_name);
        assert.equal(t.topic_id, stored.topicId);
        assert.equal(t.revision, stored.revision);
        assert.equal(t.description, stored.description);
        assert.deepEqual(t.examples, stored.examples);
      }
    }
  });
  return {
    passed: true,
    capturedCommands: receipts.length,
    profiles: 3,
    topics: 2,
    basicFallbacks: 0,
    checks,
    textAcknowledgements,
    limits: [
      'Operator-supplied evidence, not a replay against live APIs',
      'CLI version and credential-file contents are not captured',
      'Stored configuration parity, not scanning efficacy or application cutover',
    ],
  };
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  try {
    assert.equal(
      process.argv.length,
      3,
      'Usage: node scripts/validate-prod-dev-evidence.mjs /path/to/logs.tar.gz',
    );
    const archive = resolve(process.argv[2]);
    const result = validateEvidence(readArchive(archive));
    console.log(
      JSON.stringify(
        {
          archiveSha256: createHash('sha256').update(readFileSync(archive)).digest('hex'),
          ...result,
        },
        null,
        2,
      ),
    );
  } catch (error) {
    console.error(error.message);
    process.exitCode = 1;
  }
}
