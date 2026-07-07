import { readFileSync } from 'node:fs';
import { defineConfig } from 'tsup';

const pkg = JSON.parse(readFileSync(new URL('./package.json', import.meta.url), 'utf-8'));

// All runtime deps stay external — the win is bundling our ~200 source files
// into a few chunks, not vendoring node_modules.
const external = [
  ...Object.keys(pkg.dependencies ?? {}),
  ...Object.keys(pkg.optionalDependencies ?? {}),
];

export default defineConfig({
  entry: { index: 'src/index.ts', 'cli/index': 'src/cli/index.ts' },
  format: 'esm',
  target: 'es2022',
  platform: 'node',
  // Code splitting keeps the dynamic import of src/dlp lazy (its chunk only
  // loads when `runtime dlp generate` runs).
  splitting: true,
  dts: { entry: { index: 'src/index.ts' } },
  sourcemap: false,
  clean: true,
  external,
});
