import { spawnSync } from 'node:child_process';
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

const document = readFileSync('docs-site/docs/runtime/profile-migration-workflow.md', 'utf8');
const scripts = [...document.matchAll(/```bash\n([\s\S]*?)```/g)].map((match) => match[1]);
const workflow = scripts.find((script) => script.startsWith("bash <<'BASH'"));
const helper = workflow?.match(/json_check\(\) \{[\s\S]*?\n\}/)?.[0];

describe('copy-and-paste migration documentation', () => {
  it('contains syntactically valid Bash and a dependency-free validation helper', () => {
    expect(scripts).toHaveLength(4);
    expect(helper).toBeTruthy();
    expect(workflow).not.toContain('jq ');
    for (const script of scripts) {
      const result = spawnSync('bash', ['-n'], { input: script, encoding: 'utf8' });
      expect(result.status, result.stderr).toBe(0);
    }
  });

  it.each([
    ['empty', [], 0],
    ['empty', [{ profileId: 'existing' }], 1],
    ['complete', [{ complete: true }], 0],
    ['complete', [{ complete: false }], 1],
    ['verified', [{ complete: true, profiles: [{ action: 'verified' }] }], 0],
    ['verified', [{ complete: true, profiles: [{ action: 'created' }] }], 1],
    ['verified', [{ complete: true, profiles: [] }], 1],
    [
      'tenants',
      [
        { name: 'aisecurity', tsgId: '1852583913' },
        { name: 'cdot65', tsgId: '1220195158' },
      ],
      0,
    ],
    [
      'tenants',
      [
        { name: 'aisecurity', tsgId: '1852583913' },
        { name: 'cdot65', tsgId: 'wrong' },
      ],
      1,
    ],
  ])('validates %s evidence (case %#)', (mode, data, expected) => {
    const directory = mkdtempSync(join(tmpdir(), 'airs-migration-docs-'));
    try {
      writeFileSync(join(directory, 'evidence.json'), JSON.stringify(data), { mode: 0o600 });
      const result = spawnSync(
        'bash',
        ['-c', `${helper}\njson_check "$1" evidence.json`, 'documentation-test', String(mode)],
        { cwd: directory, encoding: 'utf8' },
      );
      expect(result.status, result.stderr).toBe(expected);
    } finally {
      rmSync(directory, { recursive: true, force: true });
    }
  });
});
