import type { TestCase } from './types.js';

/**
 * Parse a single RFC 4180 CSV line into fields.
 * Handles quoted fields and escaped quotes ("" → ").
 */
function parseCsvLine(line: string): string[] {
  const fields: string[] = [];
  let i = 0;

  while (i < line.length) {
    if (line[i] === '"') {
      // Quoted field
      i++; // skip opening quote
      let value = '';
      while (i < line.length) {
        if (line[i] === '"') {
          if (line[i + 1] === '"') {
            value += '"';
            i += 2;
          } else {
            i++; // skip closing quote
            break;
          }
        } else {
          value += line[i];
          i++;
        }
      }
      fields.push(value);
      // skip comma
      if (i < line.length && line[i] === ',') i++;
    } else {
      // Unquoted field
      const end = line.indexOf(',', i);
      if (end === -1) {
        fields.push(line.slice(i));
        break;
      } else {
        fields.push(line.slice(i, end));
        i = end + 1;
        // trailing comma: push empty final field
        if (i === line.length) {
          fields.push('');
        }
      }
    }
  }

  return fields;
}

/**
 * Load test cases from a CSV string.
 *
 * Expected columns: `prompt`, `expected` (boolean as "true"/"false"), `intent` (block/allow).
 * Returns TestCase[] with category set to ''.
 * Throws on missing columns, invalid/mixed intent, no true positives, or no true negatives.
 * Calls onWarning if >80% of cases are one class.
 */
export function loadPrompts(csv: string, onWarning?: (msg: string) => void): TestCase[] {
  const lines = csv.split(/\r?\n/);
  const nonEmpty = lines.filter((l) => l.trim() !== '');

  if (nonEmpty.length === 0) {
    throw new Error('CSV is empty');
  }

  const headers = parseCsvLine(nonEmpty[0]).map((h) => h.trim().toLowerCase());

  const promptIdx = headers.indexOf('prompt');
  if (promptIdx === -1) {
    throw new Error('Missing required column: prompt');
  }

  const expectedIdx = headers.indexOf('expected');
  if (expectedIdx === -1) {
    throw new Error('Missing required column: expected');
  }

  const intentIdx = headers.indexOf('intent');
  if (intentIdx === -1) {
    throw new Error('Missing required column: intent');
  }

  // Parse rows and validate intent consistency
  const rows: Array<{ prompt: string; expected: boolean; intent: string }> = [];

  for (let i = 1; i < nonEmpty.length; i++) {
    const fields = parseCsvLine(nonEmpty[i]);
    const prompt = fields[promptIdx] ?? '';
    const expected = (fields[expectedIdx] ?? '').trim().toLowerCase() === 'true';
    const intent = (fields[intentIdx] ?? '').trim().toLowerCase();
    rows.push({ prompt, expected, intent });
  }

  // Validate intent values
  const intents = new Set(rows.map((r) => r.intent));
  for (const intent of intents) {
    if (intent !== 'block' && intent !== 'allow') {
      throw new Error(`Invalid intent value: '${intent}'. Must be 'block' or 'allow'`);
    }
  }
  if (intents.size > 1) {
    throw new Error('All rows must have the same intent value');
  }

  const intent = rows[0]?.intent ?? 'block';

  // Map expected + intent → shouldTrigger
  const testCases: TestCase[] = rows.map((r) => {
    const shouldTrigger = intent === 'block' ? r.expected : !r.expected;
    return { prompt: r.prompt, expectedTriggered: shouldTrigger, category: '' };
  });

  const truePositives = testCases.filter((t) => t.expectedTriggered);
  const trueNegatives = testCases.filter((t) => !t.expectedTriggered);

  if (truePositives.length === 0) {
    throw new Error('No true-positive prompts found (all expected=false)');
  }

  if (trueNegatives.length === 0) {
    throw new Error('No true-negative prompts found (all expected=true)');
  }

  if (onWarning) {
    const total = testCases.length;
    const majorityRatio = Math.max(truePositives.length, trueNegatives.length) / total;
    if (majorityRatio > 0.8) {
      onWarning(
        `imbalanced set: ${truePositives.length} true-positive(s) vs ${trueNegatives.length} true-negative(s) (${Math.round(majorityRatio * 100)}% one class)`,
      );
    }
  }

  return testCases;
}
