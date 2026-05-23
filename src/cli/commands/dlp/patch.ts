export interface BuildMergePatchOpts {
  set?: string[];
  clear?: string[];
}

/** Build a JSON Merge Patch object from --set/--clear CLI flags. Pure. */
export function buildMergePatch(opts: BuildMergePatchOpts): Record<string, unknown> {
  const out: Record<string, unknown> = {};

  for (const entry of opts.set ?? []) {
    const eq = entry.indexOf('=');
    if (eq < 1) throw new Error(`--set expected key=value, got: ${entry}`);
    const key = entry.slice(0, eq);
    const raw = entry.slice(eq + 1);
    if (key.includes('.')) {
      throw new Error(`--set ${key}: use --body-file for nested fields`);
    }
    if (raw === 'null') {
      throw new Error(`--set ${key}=null: to clear a field, use --clear ${key}`);
    }
    out[key] = coerceValue(raw);
  }

  for (const key of opts.clear ?? []) {
    if (key.includes('.')) {
      throw new Error(`--clear ${key}: use --body-file for nested fields`);
    }
    out[key] = null;
  }

  return out;
}

function coerceValue(raw: string): unknown {
  if (raw === 'true') return true;
  if (raw === 'false') return false;
  if (raw !== '' && !Number.isNaN(Number(raw)) && /^-?\d+(\.\d+)?$/.test(raw)) {
    return Number(raw);
  }
  if (raw.startsWith('{') || raw.startsWith('[') || raw.startsWith('"')) {
    try {
      return JSON.parse(raw);
    } catch {
      return raw;
    }
  }
  return raw;
}
