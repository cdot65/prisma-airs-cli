import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

const scripts = [new URL('./publish-prerelease.mjs', import.meta.url).pathname];
const cases = [
  { name: 'prerelease success preserves latest', version: '1.2.4-forgejo.0', ok: true },
  { name: 'wrong ref denied', version: '1.2.4-forgejo.0', ref: 'refs/heads/main', ok: false },
  { name: 'existing version denied', version: '1.2.4-forgejo.0', exists: true, ok: false },
  { name: 'ordinary stable invocation denied', version: '1.2.4', ok: false },
  { name: 'explicit stable release accepted', version: '1.2.4', stable: true, ok: true },
  { name: 'stable rollback denied', version: '1.2.2', stable: true, ok: false },
  { name: 'draft stable release denied', version: '1.2.4', stable: true, draft: true, ok: false },
  { name: 'unrelated tag mutation detected', version: '1.2.4-forgejo.0', drift: true, ok: false },
  { name: 'npm failure detected', version: '1.2.4-forgejo.0', npmStatus: 1, ok: false },
];
let passed = 0;
for (const script of scripts)
  for (const fixture of cases) {
    const dir = mkdtempSync(join(tmpdir(), 'airs-publish-boundary-'));
    try {
      writeFileSync(
        join(dir, 'package.json'),
        JSON.stringify({ name: '@cdot65/prisma-airs-sdk', version: fixture.version }),
      );
      writeFileSync(
        join(dir, 'event.json'),
        JSON.stringify({
          action: 'published',
          release: {
            tag_name: `v${fixture.version}`,
            draft: fixture.draft ?? false,
            prerelease: false,
          },
        }),
      );
      const source = `
      import cp from 'node:child_process';
      import {syncBuiltinESMExports} from 'node:module';
      const f=${JSON.stringify(fixture)};
      cp.spawnSync=()=>({status:f.npmStatus??0});
      syncBuiltinESMExports();
      let calls=0;
      globalThis.fetch=async()=>({status:200,json:async()=>{
        calls++;
        const versions = calls>1 || f.exists ? {[f.version]:{dist:{integrity:'fixture'}}} : {};
        const tags={latest: calls>1&&f.stable ? f.version : '1.2.3', next:'1.3.0-beta.0'};
        if(calls>1&&!f.stable) tags['forgejo-preview']=f.version;
        if(calls>1&&f.drift) tags.next='9.0.0';
        return {versions,'dist-tags':tags};
      }});
      await import(${JSON.stringify(script)});
    `;
      const result = spawnSync(process.execPath, ['--input-type=module', '-e', source], {
        cwd: dir,
        encoding: 'utf8',
        env: {
          ...process.env,
          GITHUB_REF: fixture.ref ?? `refs/tags/v${fixture.version}`,
          AIRS_RELEASE_MODE: fixture.stable ? 'stable' : '',
          GITHUB_EVENT_NAME: fixture.stable ? 'release' : 'push',
          GITHUB_EVENT_PATH: join(dir, 'event.json'),
        },
        timeout: 10000,
      });
      assert.equal(result.status === 0, fixture.ok, `${script}: ${fixture.name}\n${result.stderr}`);
      passed++;
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  }
console.log(
  `${passed} publication boundary checks passed; npm calls mocked, no network or publication.`,
);
