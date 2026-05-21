# DLP Test-File Generator Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build `airs runtime dlp-gen`, a CLI command that generates clean carrier files and "dirty" copies with synthetic sensitive data embedded via multiple techniques, plus a `.claude/skills/dlp-test-files` skill that drives it.

**Architecture:** Framework-free, unit-tested core under `src/dlp/` (payload generator, lorem, manifest, per-format clean generators, per-format embedders, orchestrator). Thin Commander wiring under `src/cli/` registers `dlp-gen` in the existing `runtime` group. A committed skill documents when/how to invoke the command.

**Tech Stack:** TypeScript, Commander, Zod, Vitest; `pdf-lib`, `docx`, `sharp`, `piexifjs`.

---

## File Structure

```
src/dlp/
  types.ts            # PayloadValue, ManifestEntry, Technique, GenerateOptions
  rng.ts              # seeded RNG (mulberry32)
  payload.ts          # synthetic PII generator (uses rng)
  lorem.ts            # lorem ipsum generator (uses rng)
  manifest.ts         # manifest assembly + JSON write
  generate/{pdf,docx,png,jpeg,svg}.ts   # clean file bytes
  embed/{pdf,docx,png,jpeg,svg}.ts      # technique map: (bytes, payload) -> dirty bytes
  embed/extractors.ts # test-only helpers to recover embedded values (also used by --verify)
  index.ts            # orchestration: generate -> embed per technique -> write + manifest
src/dlp/__tests__/*.test.ts
src/cli/commands/runtime.ts             # MODIFY: register `dlp-gen`
src/cli/renderer/runtime.ts             # MODIFY: render summary
.claude/skills/dlp-test-files/SKILL.md  # the skill
.changeset/0008-runtime-dlp-gen.md
```

Test layout: confirm convention in Task 0 (`src/**/__tests__/*.test.ts` vs co-located). Follow whatever exists.

---

## Task 0: Scaffolding, deps, test-convention

**Files:** `package.json` (deps), confirm test layout.

- [ ] **Step 1:** Inspect existing test convention.

Run: `find src -name "*.test.ts" | head; cat vitest.config.* 2>/dev/null; grep -n '"test"' package.json`
Expected: learn where tests live and the coverage `exclude` globs. Use that location for all `*.test.ts` below.

- [ ] **Step 2:** Add runtime deps.

Run: `pnpm add pdf-lib docx sharp piexifjs && pnpm add -D @types/piexifjs`
Expected: deps land in `package.json`; `pnpm install` succeeds.

- [ ] **Step 3:** Verify build still green.

Run: `pnpm typecheck && pnpm lint`
Expected: PASS.

- [ ] **Step 4:** Commit.

```bash
git add package.json pnpm-lock.yaml
git commit -m "build: add pdf-lib, docx, sharp, piexifjs for dlp-gen"
```

---

## Task 1: Types + seeded RNG

**Files:** Create `src/dlp/types.ts`, `src/dlp/rng.ts`, test `rng.test.ts`.

- [ ] **Step 1: Write failing test** (`rng.test.ts`)

```typescript
import { describe, it, expect } from "vitest";
import { makeRng } from "../rng";

describe("makeRng", () => {
  it("is deterministic for a seed", () => {
    const a = makeRng(42); const b = makeRng(42);
    expect([a(), a(), a()]).toEqual([b(), b(), b()]);
  });
  it("differs across seeds", () => {
    expect(makeRng(1)()).not.toEqual(makeRng(2)());
  });
  it("returns floats in [0,1)", () => {
    const r = makeRng(7);
    for (let i = 0; i < 100; i++) { const v = r(); expect(v).toBeGreaterThanOrEqual(0); expect(v).toBeLessThan(1); }
  });
});
```

- [ ] **Step 2:** Run `pnpm vitest run src/dlp` → FAIL (no module).

- [ ] **Step 3: Implement**

`src/dlp/rng.ts`:
```typescript
/** Deterministic PRNG (mulberry32). Returns () => float in [0,1). */
export function makeRng(seed: number): () => number {
  let a = seed >>> 0;
  return () => {
    a |= 0; a = (a + 0x6d2b79f5) | 0;
    let t = Math.imul(a ^ (a >>> 15), 1 | a);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}
export const pick = <T>(rng: () => number, xs: T[]): T => xs[Math.floor(rng() * xs.length)];
export const digits = (rng: () => number, n: number): string =>
  Array.from({ length: n }, () => Math.floor(rng() * 10)).join("");
```

`src/dlp/types.ts`:
```typescript
export type Format = "pdf" | "png" | "jpeg" | "svg" | "docx";
export interface PayloadValue { category: string; value: string; }
export interface Technique {
  id: string; format: Format; label: string;
  embed: (clean: Buffer, payload: PayloadValue[]) => Promise<Buffer> | Buffer;
}
export interface ManifestEntry {
  format: Format; technique: string; clean: string; dirty: string; values: PayloadValue[];
}
export interface GenerateOptions {
  types: Format[]; count: number; out: string; techniques: "all" | string[]; seed?: number;
}
```

- [ ] **Step 4:** Run `pnpm vitest run src/dlp` → PASS.

- [ ] **Step 5:** Commit `feat(dlp): seeded rng + core types`.

---

## Task 2: Synthetic payload generator

**Files:** Create `src/dlp/payload.ts`, test `payload.test.ts`.

- [ ] **Step 1: Write failing test**

```typescript
import { describe, it, expect } from "vitest";
import { makePayload, luhnValid } from "../payload";

describe("makePayload", () => {
  it("is deterministic by seed", () => {
    expect(makePayload(makeRngFromSeed(5))).toEqual(makePayload(makeRngFromSeed(5)));
  });
  it("emits a luhn-valid test PAN", () => {
    const pan = find(makePayload(rng()), "credit_card").replace(/\s/g, "");
    expect(luhnValid(pan)).toBe(true);
    expect(pan.startsWith("4")).toBe(true);            // test Visa BIN family
  });
  it("uses only reserved/example ranges (no real-PII shapes)", () => {
    const all = makePayload(rng()).map(v => v.value).join(" ");
    expect(all).toMatch(/@example\.(com|org|net)/);
    expect(all).toMatch(/\b555-01\d\d\b/);
    expect(all).not.toMatch(/sk_live_/);               // no stripe push-protection trigger
  });
});
```
(Add small local `rng`/`find` helpers in the test.)

- [ ] **Step 2:** Run → FAIL.

- [ ] **Step 3: Implement** `src/dlp/payload.ts`

```typescript
import { type PayloadValue } from "./types";
import { digits, pick } from "./rng";

export function luhnValid(pan: string): boolean {
  let sum = 0, alt = false;
  for (let i = pan.length - 1; i >= 0; i--) {
    let d = pan.charCodeAt(i) - 48;
    if (alt) { d *= 2; if (d > 9) d -= 9; }
    sum += d; alt = !alt;
  }
  return pan.length > 0 && sum % 10 === 0;
}

function testPan(rng: () => number): string {
  const body = "4" + digits(rng, 14);            // Visa-family test BIN, 15 digits
  for (let c = 0; c < 10; c++) { const cand = body + c; if (luhnValid(cand)) return cand; }
  return body + "0";
}
const ssn = (rng: () => number) => `9${digits(rng, 2)}-${digits(rng, 2)}-${digits(rng, 4)}`; // 900-range never issued
const phone = (rng: () => number) => `555-01${digits(rng, 2)}`;
const email = (rng: () => number) =>
  `${pick(rng, ["john.public", "jane.doe", "sam.roe"])}.${digits(rng, 3)}@${pick(rng, ["example.com", "example.org", "example.net"])}`;
const awsKey = (rng: () => number) => `AKIA${Array.from({ length: 12 }, () => pick(rng, [..."ABCDEFGHIJKLMNOPQRSTUVWXYZ234567"])).join("")}EXAMPLE`.slice(0, 20);

export function makePayload(rng: () => number): PayloadValue[] {
  const pan = testPan(rng);
  return [
    { category: "ssn", value: ssn(rng) },
    { category: "credit_card", value: pan.replace(/(\d{4})(?=\d)/g, "$1 ").trim() },
    { category: "email", value: email(rng) },
    { category: "phone", value: phone(rng) },
    { category: "aws_key", value: awsKey(rng) },
    { category: "passport", value: `X${digits(rng, 8)}` },
    { category: "dob", value: `19${60 + Math.floor(rng() * 40)}-0${1 + Math.floor(rng() * 9)}-1${Math.floor(rng() * 9)}` },
  ];
}
```

- [ ] **Step 4:** Run → PASS. - [ ] **Step 5:** Commit `feat(dlp): synthetic payload generator`.

---

## Task 3: Lorem + manifest

**Files:** Create `src/dlp/lorem.ts`, `src/dlp/manifest.ts`, tests.

- [ ] **Step 1: Tests**

```typescript
// lorem.test.ts
import { lorem } from "../lorem";
it("returns N deterministic paragraphs", () => {
  expect(lorem(makeRng(1), 3).split("\n\n")).toHaveLength(3);
  expect(lorem(makeRng(1), 3)).toEqual(lorem(makeRng(1), 3));
});
// manifest.test.ts
import { buildManifest } from "../manifest";
it("maps dirty files to technique + values", () => {
  const m = buildManifest([{ format:"svg", technique:"meta", clean:"a.svg", dirty:"a__meta.svg", values:[{category:"ssn",value:"900-00-0000"}] }]);
  expect(m.entries[0].technique).toBe("meta");
  expect(m.count).toBe(1);
});
```

- [ ] **Step 2:** Run → FAIL.

- [ ] **Step 3: Implement**

`src/dlp/lorem.ts`: a fixed word bank + `pick`-based sentence/paragraph assembly, `lorem(rng, paras)` returns paragraphs joined by `\n\n`.

`src/dlp/manifest.ts`:
```typescript
import { writeFile } from "node:fs/promises";
import { type ManifestEntry } from "./types";
export interface Manifest { generatedAt: string; count: number; entries: ManifestEntry[]; }
export function buildManifest(entries: ManifestEntry[]): Manifest {
  return { generatedAt: new Date().toISOString(), count: entries.length, entries };
}
export const writeManifest = (path: string, m: Manifest) => writeFile(path, JSON.stringify(m, null, 2));
```

- [ ] **Step 4:** Run → PASS. - [ ] **Step 5:** Commit `feat(dlp): lorem + manifest`.

---

## Task 4: Clean generators

**Files:** Create `src/dlp/generate/{pdf,docx,png,jpeg,svg}.ts`, test `generate.test.ts`.

Each exports `async (rng) => Buffer`. Tests assert the magic bytes / structural validity:

- [ ] **Step 1: Test** (one assertion per format)

```typescript
import { pdf } from "../generate/pdf"; // etc.
it("pdf starts with %PDF", async () => expect((await pdf(makeRng(1))).subarray(0,4).toString()).toBe("%PDF"));
it("png magic", async () => expect((await png(makeRng(1))).subarray(1,4).toString()).toBe("PNG"));
it("jpeg SOI", async () => expect((await jpeg(makeRng(1))).subarray(0,2).toString("hex")).toBe("ffd8"));
it("svg root", async () => expect((await svg(makeRng(1))).toString()).toContain("<svg"));
it("docx is a zip (PK)", async () => expect((await docx(makeRng(1))).subarray(0,2).toString()).toBe("PK"));
```

- [ ] **Step 2:** Run → FAIL.

- [ ] **Step 3: Implement** each:
  - `pdf.ts`: `pdf-lib` `PDFDocument.create()`, add page(s), `drawText(lorem(rng,3))`, `.save()` → Buffer.
  - `docx.ts`: `docx` `Document` with heading + lorem paragraphs, `Packer.toBuffer`.
  - `png.ts`/`jpeg.ts`: `sharp` from a raw gradient buffer (`{create:{width,height,channels,background}}` or raw pixel `Buffer`), `.png()`/`.jpeg()` → Buffer.
  - `svg.ts`: template string with `<svg xmlns=...>` + shapes + `<title>/<desc>`; return `Buffer.from(...)`.

- [ ] **Step 4:** Run → PASS. - [ ] **Step 5:** Commit `feat(dlp): clean file generators`.

---

## Task 5: Embedders + extractors (one task per format)

`embed/extractors.ts` provides `extract<Format>(buf): string` returning all recoverable text, used by both tests and an optional `--verify`. Each `embed/<fmt>.ts` exports `Record<string, Technique>`.

For each format below: **(a)** write a test that, for every technique, embeds a known payload then asserts each value is recoverable via the extractor AND the carrier re-parses; **(b)** run → FAIL; **(c)** implement; **(d)** run → PASS; **(e)** commit.

### Task 5a: SVG (`embed/svg.ts`) — `meta`, `hidden-text`, `comment`

- `meta`: inject `<metadata>`, `<desc>`, `<title>` after `<svg …>`.
- `hidden-text`: `<text x="-9999" y="-9999">` + `<text opacity="0">` blocks.
- `comment`: `<!-- … -->` after the root.
- Extractor: strip tags / read full string; assert values present; re-parse with a quick well-formedness check (no unclosed `<svg`).
- Test asserts: each technique's output contains every payload value; output still contains `</svg>`.
- Commit `feat(dlp): svg embedders`.

### Task 5b: PNG (`embed/png.ts`) — `text-chunks`, `trailer`, `stego-lsb`

- `text-chunks`: append `tEXt`/`zTXt`/`iTXt` chunks before `IEND` (write chunk: length, type, data, CRC32).
- `trailer`: append payload bytes after the `IEND` chunk.
- `stego-lsb`: decode pixels via `sharp(buf).raw()`, write length-prefixed bytes into LSBs, re-encode `.png()`.
- Extractors: `text-chunks` → parse chunks; `trailer` → bytes after IEND; `stego-lsb` → read LSBs.
- Test: each technique recoverable; PNG signature intact; `sharp(out).metadata()` resolves.
- Commit `feat(dlp): png embedders`.

### Task 5c: JPEG (`embed/jpeg.ts`) — `exif`, `com`, `trailer`

- `exif`: `piexifjs` set `ImageDescription` + `UserComment`, `piexif.insert`.
- `com`: insert `FFFE` segment after SOI (length-prefixed).
- `trailer`: append after `FFD9` EOI.
- Extractors accordingly; test: recoverable; `sharp(out).metadata()` resolves; starts `ffd8`.
- Commit `feat(dlp): jpeg embedders`.

### Task 5d: PDF (`embed/pdf.ts`) — `meta`, `hidden-text`, `trailer`

- `meta`: `pdf-lib` load, `setTitle/setSubject/setKeywords` with payload, save.
- `hidden-text`: load, draw payload text then set invisible — push content-stream operator `3 Tr` around the text; **fallback**: draw with white color at 1pt if render-mode injection fails. Document the chosen path in a code comment.
- `trailer`: append bytes after the final `%%EOF`.
- Extractor: `meta` → re-load pdf-lib + read info; `trailer` → bytes after last `%%EOF`; `hidden-text` → search decompressed/text via `pdf-lib` low-level or a raw-string contains check for the literal.
- Test: recoverable; output starts `%PDF`; re-loads with `PDFDocument.load`.
- Commit `feat(dlp): pdf embedders`.

### Task 5e: DOCX (`embed/docx.ts`) — `core-props`, `hidden-run`, `visible`

- DOCX is a zip; use a zip lib already available (`docx` produces; for editing use `node:zlib`+manual or add `jszip` if needed — confirm in Task 0; prefer regenerating the doc with payload baked in rather than post-editing).
- Approach: regenerate the doc from lorem **plus** payload runs: `core-props` via `Document({ title, subject, keywords })`; `hidden-run` via a `TextRun({ text, vanish: true })` or white color; `visible` via a normal paragraph.
- Extractor: unzip `word/document.xml` + `docProps/core.xml`, assert values present.
- Test: recoverable; output `PK` zip; re-open via the unzip path.
- Commit `feat(dlp): docx embedders`.

---

## Task 6: Technique registry + orchestrator

**Files:** Create `src/dlp/index.ts`, test `orchestrate.test.ts`.

- [ ] **Step 1: Test**

```typescript
import { generateCorpus } from "../index";
it("writes clean+dirty+manifest and is seed-reproducible", async () => {
  const out = await mkdtemp();
  const r1 = await generateCorpus({ types:["svg"], count:1, out, techniques:"all", seed:9 });
  expect(existsSync(`${out}/clean/svg`)).toBe(true);
  expect(readdirSync(`${out}/dirty/svg`).length).toBe(3); // 3 svg techniques
  const m = JSON.parse(readFileSync(`${out}/manifest.json`,"utf8"));
  expect(m.entries.every(e => e.values.length > 0)).toBe(true);
  // reproducibility
  const out2 = await mkdtemp();
  await generateCorpus({ types:["svg"], count:1, out:out2, techniques:"all", seed:9 });
  expect(readFileSync(`${out}/dirty/svg/${readdirSync(`${out}/dirty/svg`)[0]}`)).toEqual(
    readFileSync(`${out2}/dirty/svg/${readdirSync(`${out2}/dirty/svg`)[0]}`));
});
```

- [ ] **Step 2:** Run → FAIL.

- [ ] **Step 3: Implement** `index.ts`:
  - Build `ALL_TECHNIQUES: Technique[]` from the five `embed/*` maps.
  - `generateCorpus(opts)`: for each type → `count` clean files (seeded rng per file), write to `clean/<type>/`; for each requested technique of that type → fresh payload, `embed`, write `dirty/<type>/<base>__<id>.<ext>`, push ManifestEntry; finally `writeManifest`. Return summary `{ clean, dirty, manifestPath }`.
  - Seed handling: derive per-file/per-technique sub-seeds from `seed` deterministically.

- [ ] **Step 4:** Run → PASS. - [ ] **Step 5:** Commit `feat(dlp): orchestrator + technique registry`.

---

## Task 7: CLI wiring

**Files:** Modify `src/cli/commands/runtime.ts`, `src/cli/renderer/runtime.ts`.

- [ ] **Step 1:** Add a `dlp-gen` subcommand to the `runtime` Commander group: options `--types`, `--count`, `--out`, `--techniques`, `--seed`, `--output`. Parse comma lists; default out `./temp`; call `generateCorpus`; render summary (counts per format, manifest path) via renderer; support `--output json`.

- [ ] **Step 2:** Manual smoke:

Run: `pnpm build && node dist/cli/index.js runtime dlp-gen --types svg,png --out /tmp/dlp-smoke --seed 1`
Expected: `/tmp/dlp-smoke/clean`, `/tmp/dlp-smoke/dirty`, `manifest.json` created; summary printed.

- [ ] **Step 3:** `pnpm typecheck && pnpm lint && pnpm test` → PASS (coverage met; `src/cli/**` excluded).

- [ ] **Step 4:** Commit `feat(runtime): add dlp-gen command`.

---

## Task 8: The skill

**Files:** Create `.claude/skills/dlp-test-files/SKILL.md`.

- [ ] **Step 1:** Write SKILL.md with YAML frontmatter (`name: dlp-test-files`, `description:` including triggers: "generate DLP test files", "embed sensitive data in PDF/PNG/JPEG/SVG/DOCX", "clean/dirty corpus"). Body: when to use; `airs runtime dlp-gen` invocation patterns; the technique table; output layout; how to feed `dirty/` + `manifest.json` into a scan/score loop with `airs runtime scan`. No logic duplication — point at the CLI.

- [ ] **Step 2:** Sanity: confirm frontmatter parses (name + description present, description < ~1024 chars).

- [ ] **Step 3:** Commit `feat(skill): add dlp-test-files skill`.

---

## Task 9: Docs + changeset + PR

- [ ] **Step 1:** Add `.changeset/0008-runtime-dlp-gen.md` (minor): "Add `airs runtime dlp-gen` to generate clean + dirty DLP test corpora across PDF/PNG/JPEG/SVG/DOCX, plus a dlp-test-files skill."
- [ ] **Step 2:** Update `AGENTS.md` and `docs/` (runtime section + reference/cli-commands) with the new command. `mkdocs build` clean.
- [ ] **Step 3:** `pnpm test && pnpm lint && pnpm typecheck` → PASS.
- [ ] **Step 4:** Commit, push branch `cdot65/runtime-dlp-gen`, open PR.

---

## Self-Review

- **Spec coverage:** command/flags (T7), output layout+manifest (T3,T6), all 15 techniques (T5a–e), randomized seeded payload (T1,T2,T6), skill (T8), deps (T0), testing (every task), docs/changeset (T9). ✓
- **Placeholders:** none — each embedder task names its exact techniques + extractor + assertions. Note T0 confirms test-dir + whether a zip lib is needed for docx (decision: regenerate doc rather than post-edit, avoiding extra dep).
- **Type consistency:** `Technique.embed(clean, payload)`, `makePayload(rng)`, `generateCorpus(GenerateOptions)`, `buildManifest(entries)` used consistently across tasks. ✓
