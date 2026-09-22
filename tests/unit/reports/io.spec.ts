import { mkdtemp, readdir, readFile, rm, stat, symlink, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { writeReportFile } from '../../../src/reports/io.js';

describe('private no-clobber report publication', () => {
  let directory: string;
  beforeEach(async () => {
    directory = await mkdtemp(join(tmpdir(), 'airs-report-test-'));
  });
  afterEach(async () => {
    await rm(directory, { recursive: true, force: true });
  });

  it('publishes a complete private report and cleans its temporary file', async () => {
    const destination = join(directory, 'daily.html');
    await writeReportFile(destination, '<html>complete</html>');
    expect(await readFile(destination, 'utf8')).toBe('<html>complete</html>');
    expect((await stat(destination)).mode & 0o777).toBe(0o600);
    expect(await readdir(directory)).toEqual(['daily.html']);
  });

  it('refuses to overwrite either an existing report or a symlink to credentials', async () => {
    const target = join(directory, 'config.json');
    await writeFile(target, 'SECRET');
    const link = join(directory, 'report.html');
    await symlink(target, link);
    await expect(writeReportFile(target, 'new')).rejects.toMatchObject({ code: 'EEXIST' });
    await expect(writeReportFile(link, 'new')).rejects.toMatchObject({ code: 'EEXIST' });
    expect(await readFile(target, 'utf8')).toBe('SECRET');
    expect((await readdir(directory)).sort()).toEqual(['config.json', 'report.html']);
  });

  it('allows exactly one concurrent writer to claim a new destination', async () => {
    const destination = join(directory, 'report.md');
    const results = await Promise.allSettled([
      writeReportFile(destination, 'first'),
      writeReportFile(destination, 'second'),
    ]);
    expect(results.filter((result) => result.status === 'fulfilled')).toHaveLength(1);
    expect(['first', 'second']).toContain(await readFile(destination, 'utf8'));
    expect(await readdir(directory)).toEqual(['report.md']);
  });

  it('fails safely when the output directory is absent', async () => {
    await expect(
      writeReportFile(join(directory, 'missing', 'report.md'), 'content'),
    ).rejects.toMatchObject({ code: 'ENOENT' });
  });
});
