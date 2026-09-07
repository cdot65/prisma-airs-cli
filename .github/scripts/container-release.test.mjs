import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';
import { load } from 'js-yaml';
import { promotionAliases, releaseVersion } from './container-release.mjs';

const refs = (...versions) => versions.map((version) => `refs/tags/v${version}`);

test('a matching stable tag is required for publication', () => {
  assert.equal(releaseVersion('tag', 'v4.3.1', '4.3.1'), '4.3.1');
  for (const args of [
    ['branch', 'main', '4.3.1'],
    ['tag', 'v4.3.0', '4.3.1'],
    ['tag', '4.3.1', '4.3.1'],
    ['tag', 'v4.3.1-beta.1', '4.3.1-beta.1'],
    ['tag', 'v04.3.1', '04.3.1'],
    ['tag', 'v4.3', '4.3'],
    ['tag', 'v4.3.1\n', '4.3.1\n'],
  ])
    assert.throws(() => releaseVersion(...args), /stable vX.Y.Z tag/);
});

test('latest stable release owns minor and latest aliases', () => {
  assert.deepEqual(promotionAliases('4.3.1', refs('4.2.2', '4.3.0', '4.3.1')), ['4.3', 'latest']);
});

test('older patch cannot roll back either alias', () => {
  assert.deepEqual(promotionAliases('4.3.0', refs('4.3.1', '4.3.0')), []);
});

test('maintenance release may advance its own minor but never latest', () => {
  assert.deepEqual(promotionAliases('4.2.3', refs('4.3.1', '4.2.2', '4.2.3')), ['4.2']);
});

test('numeric ordering is independent of API order and tag creation time', () => {
  assert.deepEqual(promotionAliases('4.9.9', refs('4.10.0', '4.9.9')), ['4.9']);
  assert.deepEqual(promotionAliases('4.3.9', refs('4.3.10', '4.3.9')), []);
  assert.deepEqual(promotionAliases('9.3.1', refs('10.0.0', '9.3.1')), ['9.3']);
});

test('prereleases, branches and malformed tags do not reserve production aliases', () => {
  assert.deepEqual(
    promotionAliases('4.3.1', [...refs('5.0.0-beta.1', '99.01.0', '4.3.1'), 'refs/heads/v9.0.0']),
    ['4.3', 'latest'],
  );
});

test('missing or invalid release metadata fails closed', () => {
  assert.throws(() => promotionAliases('4.3.1', refs('4.3.0')), /tag missing/);
  assert.throws(() => promotionAliases('4.3.1-beta.1', refs('4.3.1')), /stable release/);
});

test('an older queued rerun reevaluates tags after the newer release', () => {
  const beforeNewTag = refs('4.3.0');
  assert.deepEqual(promotionAliases('4.3.0', beforeNewTag), ['4.3', 'latest']);
  const afterNewTag = [...beforeNewTag, ...refs('4.3.1')];
  assert.deepEqual(promotionAliases('4.3.1', afterNewTag), ['4.3', 'latest']);
  assert.deepEqual(promotionAliases('4.3.0', afterNewTag), []);
});

test('workflow separates exact-version publication from serialized post-build promotion', () => {
  const workflow = load(
    readFileSync(new URL('../workflows/docker-publish.yml', import.meta.url), 'utf8'),
  );
  const build = workflow.jobs['build-and-push'];
  const promotion = workflow.jobs['promote-aliases'];
  const metadata = build.steps.find((step) => step.id === 'meta');
  assert.equal(metadata.with.flavor.trim(), 'latest=false');
  // biome-ignore lint/suspicious/noTemplateCurlyInString: literal GitHub Actions expression under test
  assert.equal(metadata.with.tags.trim(), 'type=raw,value=${{ steps.release.outputs.version }}');
  assert.deepEqual(promotion.needs, ['build-and-push', 'verify-container']);
  const verification = workflow.jobs['verify-container'];
  assert.equal(verification.needs, 'build-and-push');
  assert.deepEqual(verification.strategy.matrix.platform, ['linux/amd64', 'linux/arm64']);
  const smoke = verification.steps.find((step) => step.name?.startsWith('Verify exact'));
  assert.match(smoke.run, /--network none/);
  assert.match(smoke.run, /IMAGE_DIGEST/);
  assert.match(smoke.run, /readonly/);
  assert.deepEqual(promotion.concurrency, {
    group: 'prisma-airs-cli-container-alias-promotion',
    'cancel-in-progress': false,
  });
  const promote = promotion.steps.find((step) => step.name === 'Promote eligible aliases');
  assert.match(promote.with.script, /github\.paginate\(github\.rest\.git\.listMatchingRefs/);
  assert.match(promote.with.script, /promotionAliases/);
  assert.match(promote.with.script, /IMAGE_DIGEST/);
  assert.match(promote.with.script, /execFileSync/);
});

test('production audit gates CI, npm publication, and container publication', () => {
  for (const workflowName of ['ci', 'publish', 'docker-publish']) {
    const workflow = load(
      readFileSync(new URL(`../workflows/${workflowName}.yml`, import.meta.url), 'utf8'),
    );
    assert.ok(
      Object.values(workflow.jobs).some((job) =>
        job.steps?.some((step) => step.run === 'pnpm audit:prod'),
      ),
    );
  }
});
