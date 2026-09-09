import { spawnSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

const paths = [
  'docs-site/docs/runtime/prod-dev-migration.md',
  'docs-site/docs/runtime/profile-migration-workflow.md',
  'docs-site/docs/runtime/dlp/tenant-auth-recovery.md',
];
const documents = paths.map((path) => readFileSync(path, 'utf8'));

describe('public migration guides use direct airs commands', () => {
  it.each(paths)('%s contains no capture harness or external scripting', (path) => {
    const doc = readFileSync(path, 'utf8');
    expect(doc).not.toMatch(
      /migration-helpers\.bash|create_test_dlp|json_check|airs_local|load_tenant_ids/,
    );
    const blocks = [...doc.matchAll(/```bash\n([\s\S]*?)```/g)].map((m) => m[1]);
    expect(blocks.length).toBeGreaterThan(0);
    for (const block of blocks) {
      expect(block).not.toMatch(/\$|\b(?:node|npm|curl|source|export|capture|bash|unset)\s/);
      // Inline JSON is a single CLI argument, not another executable.
      const withoutBodies = block.replace(/--body '[\s\S]*?'/g, '--body fixture');
      for (const line of withoutBodies
        .replace(/\\\n\s*/g, ' ')
        .split('\n')
        .filter(Boolean)) {
        expect(line).toMatch(/^airs /);
        expect(line.replace(/"<[A-Z_]+>"/g, 'placeholder')).not.toMatch(/[;|&<>]/);
      }
      expect(spawnSync('bash', ['-n'], { input: block, encoding: 'utf8' }).status).toBe(0);
    }
  });

  it('keeps the destination pin and read-only preview in every restore workflow', () => {
    for (const doc of documents) {
      const calls = [
        ...doc.replace(/\\\n\s*/g, ' ').matchAll(/^airs runtime profiles restore [^\n]*/gm),
      ].map((m) => m[0]);
      expect(calls.length).toBeGreaterThan(0);
      for (const call of calls.filter((call) => !call.includes('--help'))) {
        expect(call).toContain('--expect-tsg');
        expect(call).toContain('--on-missing-dlp');
      }
      expect(calls.some((call) => call.includes('--dry-run'))).toBe(true);
    }
    expect(documents[0]).toContain('not a verbatim replay');
    expect(documents[0]).toContain('not globally read-only');
  });

  it('matches every documented command and option to actual CLI help without executing actions', () => {
    const checked = new Map<string, string>();
    for (const doc of documents) {
      for (const match of doc.matchAll(/```bash\n([\s\S]*?)```/g)) {
        const result = spawnSync(
          '/bin/bash',
          ['-c', `airs() { printf '%s\\0' "$@"; printf '\\036'; }\n${match[1]}`],
          { encoding: 'utf8', env: { PATH: '' }, timeout: 1000 },
        );
        expect(result.status, result.stderr).toBe(0);
        for (const command of result.stdout.split('\x1e').filter(Boolean)) {
          const args = command.split('\0').slice(0, -1);
          const depth =
            args[0] === 'tenant' ? 2 : args[0] === 'runtime' ? (args[1] === 'dlp' ? 4 : 3) : 0;
          const route = args.slice(0, depth);
          const key = route.join(' ');
          if (!checked.has(key)) {
            const help = spawnSync(
              process.execPath,
              [
                '--import',
                resolve('node_modules/tsx/dist/loader.mjs'),
                resolve('src/cli/index.ts'),
                ...route,
                '--help',
              ],
              {
                encoding: 'utf8',
                timeout: 10000,
                env: { PATH: process.env.PATH, DOTENV_CONFIG_PATH: '/dev/null', NO_COLOR: '1' },
              },
            );
            expect(help.status, help.stderr).toBe(0);
            expect(help.stdout).toContain('Usage:');
            checked.set(key, help.stdout);
          }
          for (const flag of args.filter((arg) => arg.startsWith('--'))) {
            expect(checked.get(key), `${key}: ${flag}`).toContain(flag);
          }
        }
      }
    }
    expect(checked.size).toBeGreaterThan(12);
  }, 60000);
});
