import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { readFileSync, writeFileSync } from 'node:fs';

// Stable publication requires a separate, explicitly published Forgejo release.
const pkg = JSON.parse(readFileSync('package.json', 'utf8'));
assert.match(pkg.name, /^@cdot65\/prisma-airs-(sdk|cli)$/);
const stable = process.env.AIRS_RELEASE_MODE === 'stable';
if (stable) {
  assert.match(pkg.version, /^\d+\.\d+\.\d+$/);
  assert.equal(process.env.GITHUB_EVENT_NAME, 'release');
  const event = JSON.parse(readFileSync(process.env.GITHUB_EVENT_PATH, 'utf8'));
  assert.equal(event.action, 'published');
  assert.equal(event.release.tag_name, `v${pkg.version}`);
  assert.equal(event.release.draft, false);
  assert.equal(event.release.prerelease, false);
} else {
  assert.match(pkg.version, /^\d+\.\d+\.\d+-forgejo\.\d+$/);
}
assert.equal(process.env.GITHUB_REF ?? process.env.FORGEJO_REF, `refs/tags/v${pkg.version}`);
const registry = 'https://registry.npmjs.org';
const channel = stable ? 'latest' : 'forgejo-preview';
const url = `${registry}/${encodeURIComponent(pkg.name)}`;
const response = await fetch(url);
assert.equal(response.status, 200, 'Cannot snapshot existing registry state');
const before = await response.json();
assert.ok(!before.versions[pkg.version], 'Version already exists; never overwrite');
if (stable && before['dist-tags'].latest) {
  const oldParts = before['dist-tags'].latest.split('.').map(Number);
  const newParts = pkg.version.split('.').map(Number);
  assert.ok(oldParts.every(Number.isInteger), 'Unexpected existing latest version');
  const difference = newParts
    .map((value, index) => value - oldParts[index])
    .find((value) => value !== 0);
  assert.ok(difference > 0, 'Stable publication cannot roll back latest');
}
const result = spawnSync(
  'npm',
  ['publish', '--ignore-scripts', '--access', 'public', '--tag', channel, '--registry', registry],
  { stdio: 'inherit', env: process.env },
);
assert.equal(result.status, 0, 'npm publish failed');
let after;
for (let i = 0; i < 120; i++) {
  const r = await fetch(url, { headers: { 'cache-control': 'no-cache' } });
  assert.equal(r.status, 200);
  after = await r.json();
  if (after.versions[pkg.version]) break;
  await new Promise((resolve) => setTimeout(resolve, 5000));
}
assert.ok(after.versions[pkg.version], 'Published version not visible');
assert.equal(after['dist-tags'][channel], pkg.version);
for (const [tag, version] of Object.entries(before['dist-tags'])) {
  if (tag !== channel)
    assert.equal(after['dist-tags'][tag], version, `Unexpected dist-tag change: ${tag}`);
}
writeFileSync(
  'release-receipt.json',
  `${JSON.stringify({ name: pkg.name, version: pkg.version, before: before['dist-tags'], after: after['dist-tags'], integrity: after.versions[pkg.version].dist.integrity }, null, 2)}\n`,
);
console.log('Publication verified; all unrelated dist-tags preserved.');
