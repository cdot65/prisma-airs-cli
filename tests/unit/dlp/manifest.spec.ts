import { mkdtemp, readFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';
import { buildManifest, writeManifest } from '../../../src/dlp/manifest.js';
import type { ManifestEntry } from '../../../src/dlp/types.js';

const entry: ManifestEntry = {
  format: 'svg',
  technique: 'meta',
  clean: 'a.svg',
  dirty: 'a__meta.svg',
  values: [{ category: 'ssn', value: '900-00-0000' }],
};

describe('buildManifest', () => {
  it('maps dirty files to technique + values', () => {
    const m = buildManifest([entry]);
    expect(m.count).toBe(1);
    expect(m.entries[0].technique).toBe('meta');
    expect(m.entries[0].values[0].category).toBe('ssn');
    expect(typeof m.generatedAt).toBe('string');
  });
});

describe('writeManifest', () => {
  it('writes valid JSON to disk', async () => {
    const dir = await mkdtemp(join(tmpdir(), 'dlp-manifest-'));
    const path = join(dir, 'manifest.json');
    await writeManifest(path, buildManifest([entry]));
    const parsed = JSON.parse(await readFile(path, 'utf8'));
    expect(parsed.count).toBe(1);
    expect(parsed.entries[0].dirty).toBe('a__meta.svg');
  });
});
