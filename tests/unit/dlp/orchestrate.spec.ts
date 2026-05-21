import { existsSync } from 'node:fs';
import { mkdtemp, readdir, readFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';
import { generateCorpus } from '../../../src/dlp/index.js';

describe('generateCorpus', () => {
  it('writes clean + dirty + manifest across all formats and techniques', async () => {
    const out = await mkdtemp(join(tmpdir(), 'dlp-corpus-'));
    const summary = await generateCorpus({
      types: ['svg', 'png', 'jpeg', 'pdf', 'docx'],
      count: 1,
      out,
      techniques: 'all',
      seed: 9,
    });
    expect(summary.clean).toBe(5);
    expect(summary.dirty).toBe(15); // 3 techniques x 5 formats
    expect(existsSync(summary.manifestPath)).toBe(true);

    const manifest = JSON.parse(await readFile(summary.manifestPath, 'utf8'));
    expect(manifest.entries).toHaveLength(15);
    expect(manifest.entries.every((e: { values: unknown[] }) => e.values.length > 0)).toBe(true);
    expect((await readdir(join(out, 'dirty', 'svg'))).length).toBe(3);
    expect((await readdir(join(out, 'clean', 'png'))).length).toBe(1);
  });

  it('honors a technique filter', async () => {
    const out = await mkdtemp(join(tmpdir(), 'dlp-filter-'));
    const summary = await generateCorpus({
      types: ['png'],
      count: 1,
      out,
      techniques: ['stego-lsb'],
      seed: 3,
    });
    expect(summary.dirty).toBe(1);
    expect((await readdir(join(out, 'dirty', 'png')))[0]).toContain('__stego-lsb');
  });

  it('is byte-reproducible for a seed (svg)', async () => {
    const a = await mkdtemp(join(tmpdir(), 'dlp-a-'));
    const b = await mkdtemp(join(tmpdir(), 'dlp-b-'));
    await generateCorpus({ types: ['svg'], count: 1, out: a, techniques: 'all', seed: 7 });
    await generateCorpus({ types: ['svg'], count: 1, out: b, techniques: 'all', seed: 7 });
    const files = (await readdir(join(a, 'dirty', 'svg'))).sort();
    expect(files).toEqual((await readdir(join(b, 'dirty', 'svg'))).sort());
    for (const f of files) {
      expect(await readFile(join(a, 'dirty', 'svg', f))).toEqual(
        await readFile(join(b, 'dirty', 'svg', f)),
      );
    }
  });
});
