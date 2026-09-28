import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { readFileSync, writeFileSync } from 'node:fs';

// Only migration prereleases are authorized; stable promotion remains separate.
const pkg = JSON.parse(readFileSync('package.json', 'utf8'));
assert.match(pkg.name, /^@cdot65\/prisma-airs-(sdk|cli)$/);
assert.match(pkg.version, /^\d+\.\d+\.\d+-forgejo\.\d+$/);
assert.equal(process.env.GITHUB_REF ?? process.env.FORGEJO_REF, `refs/tags/v${pkg.version}`);
const registry = 'https://registry.npmjs.org';
const channel = 'forgejo-preview';
const url = `${registry}/${encodeURIComponent(pkg.name)}`;
const response = await fetch(url);
assert.equal(response.status, 200, 'Cannot snapshot existing registry state');
const before = await response.json();
assert.ok(!before.versions[pkg.version], 'Version already exists; never overwrite');
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
console.log('Prerelease published; all pre-existing non-preview tags preserved.');
