import type { TestCase } from './types.js';

/**
 * Parse a single RFC 4180 CSV line into fields.
 * Handles quoted fields and escaped quotes ("" → ").
 */
function parseCsvLine(line: string): string[] {
  const fields: string[] = [];
  let i = 0;

  while (i <= line.length) {
    if (i === line.length) {
      fields.push('');
      break;
    }

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
      }
    }
  }

  return fields;
}

/**
 * Load test cases from a CSV string.
 *
 * Expected columns: `prompt`, `expected` (boolean as "true"/"false").
 * Returns TestCase[] with category set to ''.
 * Throws on missing columns, no true positives, or no true negatives.
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

  const testCases: TestCase[] = [];

  for (let i = 1; i < nonEmpty.length; i++) {
    const fields = parseCsvLine(nonEmpty[i]);
    const prompt = fields[promptIdx] ?? '';
    const expectedRaw = (fields[expectedIdx] ?? '').trim().toLowerCase();
    const expectedTriggered = expectedRaw === 'true';

    testCases.push({ prompt, expectedTriggered, category: '' });
  }

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
