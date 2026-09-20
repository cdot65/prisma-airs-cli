/** Parse the data-only Python repr subset used by AIRS message exports; never eval. */
function pythonLiteral(source: string): unknown {
  let i = 0;
  let nodes = 0;
  const fail = (): never => {
    throw new Error('Unsupported exported literal');
  };
  const space = () => {
    while (/\s/.test(source[i] ?? '') && i < source.length) i++;
  };
  function string(): string {
    const quote = source[i++];
    let result = '';
    while (i < source.length) {
      const ch = source[i++];
      if (ch === quote) return result;
      if (ch === '\n' || ch === '\r') fail();
      if (ch !== '\\') {
        result += ch;
        continue;
      }
      const esc = source[i++];
      const simple: Record<string, string> = {
        a: '\x07',
        b: '\b',
        f: '\f',
        n: '\n',
        r: '\r',
        t: '\t',
        v: '\v',
        '\\': '\\',
        "'": "'",
        '"': '"',
      };
      if (Object.hasOwn(simple, esc)) {
        result += simple[esc];
        continue;
      }
      const width = esc === 'x' ? 2 : esc === 'u' ? 4 : esc === 'U' ? 8 : 0;
      if (width) {
        const digits = source.slice(i, i + width);
        if (digits.length !== width || !/^[\da-f]+$/i.test(digits)) fail();
        const code = Number.parseInt(digits, 16);
        if (code > 0x10ffff) fail();
        result += String.fromCodePoint(code);
        i += width;
        continue;
      }
      if (/[0-7]/.test(esc ?? '')) {
        let digits = esc;
        while (digits.length < 3 && /[0-7]/.test(source[i] ?? '')) digits += source[i++];
        result += String.fromCodePoint(Number.parseInt(digits, 8));
        continue;
      }
      if (esc === undefined) fail();
      result += `\\${esc}`;
    }
    return fail();
  }
  function value(depth: number): unknown {
    if (++nodes > 50_000 || depth > 64) fail();
    space();
    const ch = source[i];
    if (ch === "'" || ch === '"') return string();
    if (ch === '{' || ch === '[') {
      i++;
      const object: Record<string, unknown> = Object.create(null);
      const list: unknown[] = [];
      const end = ch === '{' ? '}' : ']';
      space();
      while (source[i] !== end) {
        if (ch === '{') {
          if (source[i] !== "'" && source[i] !== '"') fail();
          const key = string();
          space();
          if (source[i++] !== ':' || Object.hasOwn(object, key)) fail();
          object[key] = value(depth + 1);
        } else list.push(value(depth + 1));
        space();
        if (source[i] === end) break;
        if (source[i++] !== ',') fail();
        space();
      }
      i++;
      return ch === '{' ? object : list;
    }
    for (const [token, literal] of [
      ['True', true],
      ['False', false],
      ['None', null],
    ] as const) {
      if (source.startsWith(token, i)) {
        i += token.length;
        return literal;
      }
    }
    const match = source.slice(i).match(/^-?(?:0|[1-9]\d*)(?:\.\d+)?(?:[eE][+-]?\d+)?/);
    if (!match) return fail();
    i += match[0].length;
    const number = Number(match[0]);
    if (!Number.isFinite(number)) fail();
    return number;
  }
  const result = value(0);
  space();
  if (i !== source.length) fail();
  return result;
}

export function decodeContainer(raw: string): [unknown, string] {
  if (raw.length > 1_000_000) return [raw, 'plain'];
  try {
    return [JSON.parse(raw), 'json'];
  } catch {
    /* Try AIRS Python repr next. */
  }
  try {
    return [pythonLiteral(raw), 'python_literal'];
  } catch {
    return [raw, 'plain'];
  }
}

/** Extract only explicit A2A message envelopes, preserving literal JSON in text parts. */
export function messageText(value: unknown): [string, string] | null {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return null;
  const message = value as Record<string, unknown>;
  if (message.kind !== 'message' || !Object.hasOwn(message, 'parts')) return null;
  // Prompt envelopes can use either role; response strings are never unwrapped.
  const parts = message.parts;
  if (
    !Array.isArray(parts) ||
    parts.some(
      (part) =>
        !part || typeof part !== 'object' || part.kind !== 'text' || typeof part.text !== 'string',
    )
  )
    return ['', 'a2a_unsupported_parts'];
  return [parts.map((part) => part.text).join('\n'), 'a2a_text'];
}
