import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';

describe('Gateway dashboard documented live evidence', () => {
  const page = readFileSync('docs-site/docs/cli/aigateway/dashboard-examples.md', 'utf8');
  const stdout = `${page.split('```markdown\n')[1].split('\n```\n')[0]}\n`;
  it('preserves the byte-exact captured CLI stdout', () => {
    expect(createHash('sha256').update(stdout).digest('hex')).toBe(
      'a2cbfdfc6ad9b62f24e2868677a65f23b296fecdf4197f33876d0a38c4104f4d',
    );
    expect(stdout).toContain('| Server total | 1431 |');
    expect(stdout).toContain('| Unique collected | 1431 |');
    expect(stdout.match(/\| complete \|/g)).toHaveLength(25);
  });
  it('distinguishes test output, historical telemetry and operational risk', () => {
    const shell = page.split('```bash\n')[1].split('\n```')[0];
    expect(
      shell
        .split('\n')
        .slice(0, -1)
        .every((line) => line.endsWith(' \\') && !line.endsWith('\\\\')),
    ).toBe(true);
    expect(page).toContain('5/5 live E2E');
    expect(page).toContain('not a failing CLI test');
    expect(page).toContain('narrower 174-row window');
    expect(stdout).toContain('Success-status responses appear in error analytics');
    expect(stdout).not.toMatch(/Bearer |PRIVATE-CANARY|metadataValue|api_key_id/);
  });
});
