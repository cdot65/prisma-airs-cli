import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { appendFileSync, readFileSync } from 'node:fs';
import { promotionAliases, releaseVersion } from '../../.github/scripts/container-release.mjs';

const pkg = JSON.parse(readFileSync('package.json', 'utf8'));
const eventName = process.env.GITHUB_EVENT_NAME;
const stable = /^\d+\.\d+\.\d+$/.test(pkg.version);
if (stable) {
  assert.equal(eventName, 'release');
  const event = JSON.parse(readFileSync(process.env.GITHUB_EVENT_PATH, 'utf8'));
  assert.equal(event.action, 'published');
  assert.equal(event.release.prerelease, false);
  assert.equal(event.release.draft, false);
  releaseVersion('tag', event.release.tag_name, pkg.version);
} else {
  assert.match(pkg.version, /^\d+\.\d+\.\d+-forgejo\.\d+$/);
  assert.ok(
    process.env.GITHUB_REF === `refs/tags/v${pkg.version}` ||
      (eventName === 'workflow_dispatch' && process.env.GITHUB_REF === 'refs/heads/main'),
  );
}
const image = 'registry.cdot.io/prisma-airs/prisma-airs-cli';
if (process.argv[2] === 'promote') {
  if (stable) {
    const digest = process.env.IMAGE_DIGEST;
    assert.match(digest, /^sha256:[a-f0-9]{64}$/);
    const refs = execFileSync('git', ['ls-remote', '--tags', 'origin'], { encoding: 'utf8' })
      .trim()
      .split('\n')
      .map((row) => row.split(/\s+/)[1]);
    for (const alias of promotionAliases(pkg.version, refs)) {
      execFileSync(
        'docker',
        ['buildx', 'imagetools', 'create', '--tag', `${image}:${alias}`, `${image}@${digest}`],
        { stdio: 'inherit' },
      );
      const actual = execFileSync(
        'docker',
        [
          'buildx',
          'imagetools',
          'inspect',
          `${image}:${alias}`,
          '--format',
          '{{.Manifest.Digest}}',
        ],
        { encoding: 'utf8' },
      ).trim();
      assert.equal(actual, digest, 'Promoted alias digest mismatch');
    }
  } else {
    console.log('Prerelease verified; no stable container aliases changed.');
  }
} else {
  appendFileSync(process.env.GITHUB_ENV, `IMAGE=${image}:${pkg.version}\n`);
}
