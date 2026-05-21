import { mkdir, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { techniquesByFormat } from './embed/index.js';
import { EXT, generators } from './generate/index.js';
import { buildManifest, writeManifest } from './manifest.js';
import { makePayload } from './payload.js';
import { makeRng, subSeed } from './rng.js';
import type { GenerateOptions, ManifestEntry } from './types.js';

export interface GenerateSummary {
  clean: number;
  dirty: number;
  out: string;
  manifestPath: string;
  seed: number;
  byFormat: Record<string, { clean: number; dirty: number }>;
}

/** Generate a clean/dirty DLP test corpus and a manifest describing it. */
export async function generateCorpus(opts: GenerateOptions): Promise<GenerateSummary> {
  const seed = opts.seed ?? Math.floor(Math.random() * 0x7fffffff);
  const entries: ManifestEntry[] = [];
  const byFormat: Record<string, { clean: number; dirty: number }> = {};
  let cleanCount = 0;
  let typeIndex = 0;

  for (const fmt of opts.types) {
    const ext = EXT[fmt];
    const cleanDir = join(opts.out, 'clean', fmt);
    const dirtyDir = join(opts.out, 'dirty', fmt);
    await mkdir(cleanDir, { recursive: true });
    await mkdir(dirtyDir, { recursive: true });
    byFormat[fmt] = { clean: 0, dirty: 0 };

    const wanted = techniquesByFormat[fmt].filter(
      (t) => opts.techniques === 'all' || opts.techniques.includes(t.id),
    );

    for (let i = 0; i < opts.count; i++) {
      const fileSeed = subSeed(seed, typeIndex * 1000 + i);
      const base = `${fmt}-${fileSeed.toString(16).padStart(8, '0')}`;
      const cleanBuf = Buffer.from(await generators[fmt](makeRng(fileSeed)));
      const cleanPath = join(cleanDir, `${base}.${ext}`);
      await writeFile(cleanPath, cleanBuf);
      cleanCount++;
      byFormat[fmt].clean++;

      let tIdx = 0;
      for (const technique of wanted) {
        const payload = makePayload(makeRng(subSeed(fileSeed, 7919 + tIdx)));
        const dirtyBuf = Buffer.from(await technique.embed(cleanBuf, payload));
        const dirtyPath = join(dirtyDir, `${base}__${technique.id}.${ext}`);
        await writeFile(dirtyPath, dirtyBuf);
        entries.push({
          format: fmt,
          technique: technique.id,
          clean: cleanPath,
          dirty: dirtyPath,
          values: payload,
        });
        byFormat[fmt].dirty++;
        tIdx++;
      }
    }
    typeIndex++;
  }

  const manifestPath = join(opts.out, 'manifest.json');
  await writeManifest(manifestPath, buildManifest(entries));
  return { clean: cleanCount, dirty: entries.length, out: opts.out, manifestPath, seed, byFormat };
}
