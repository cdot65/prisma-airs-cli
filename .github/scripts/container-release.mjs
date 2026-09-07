import { appendFileSync, readFileSync } from 'node:fs';
import { pathToFileURL } from 'node:url';

const STABLE_VERSION = /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/;

/** Only an exact stable release tag may publish a production container. */
export function releaseVersion(refType, refName, packageVersion) {
  if (
    refType !== 'tag' ||
    refName !== `v${packageVersion}` ||
    !STABLE_VERSION.test(packageVersion)
  ) {
    throw new Error('Container publication requires a stable vX.Y.Z tag matching package.json.');
  }
  return packageVersion;
}

function compare(a, b) {
  const left = a.split('.').map(BigInt);
  const right = b.split('.').map(BigInt);
  for (let index = 0; index < left.length; index++) {
    if (left[index] !== right[index]) return left[index] > right[index] ? 1 : -1;
  }
  return 0;
}

/**
 * Evaluate AFTER the build, inside the single alias-promotion concurrency group.
 * Tag enumeration (not creation time, lexicographic order or a queued snapshot)
 * prevents an older build/manual rerun from rolling production aliases backwards.
 * A newer stable tag reserves its aliases even if its image is not ready yet.
 */
export function promotionAliases(version, refs) {
  if (!STABLE_VERSION.test(version)) throw new Error('Expected a stable release version.');
  const versions = refs
    .filter((ref) => ref.startsWith('refs/tags/v'))
    .map((ref) => ref.slice('refs/tags/v'.length))
    .filter((candidate) => STABLE_VERSION.test(candidate));
  if (!versions.includes(version))
    throw new Error('Release tag missing from authoritative tag list.');
  const minor = version.split('.').slice(0, 2).join('.');
  const aliases = [];
  if (
    !versions.some(
      (candidate) => candidate.startsWith(`${minor}.`) && compare(candidate, version) > 0,
    )
  ) {
    aliases.push(minor);
  }
  if (!versions.some((candidate) => compare(candidate, version) > 0)) aliases.push('latest');
  return aliases;
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  const pkg = JSON.parse(readFileSync('package.json', 'utf8'));
  const version = releaseVersion(
    process.env.GITHUB_REF_TYPE,
    process.env.GITHUB_REF_NAME,
    pkg.version,
  );
  if (!process.env.GITHUB_OUTPUT) throw new Error('Missing GitHub Actions output file.');
  appendFileSync(process.env.GITHUB_OUTPUT, `version=${version}\n`);
}
