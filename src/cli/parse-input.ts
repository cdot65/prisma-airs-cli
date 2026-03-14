/**
 * Parse an input file into an array of prompt strings.
 *
 * - `.csv` files: parses CSV, extracts the `prompt` column by header name.
 *   Handles quoted fields, escaped quotes, and commas within quotes.
 * - `.txt` / extensionless: splits on newlines, trims, filters blanks.
 */
export function parseInputFile(content: string, filePath: string): string[] {
  const isCsv = /\.csv$/i.test(filePath);
  if (isCsv) {
    return parseCsv(content);
  }
  return content
    .split('\n')
    .map((l) => l.trim())
    .filter((l) => l.length > 0);
}

/** Parse CSV content and extract the `prompt` column values. */
function parseCsv(content: string): string[] {
  const rows = parseCsvRows(content);
  if (rows.length === 0) return [];

  const header = rows[0];
  const promptIdx = header.findIndex((h) => h.trim().toLowerCase() === 'prompt');
  if (promptIdx === -1) {
    throw new Error('No "prompt" column found in CSV header');
  }

  return rows
    .slice(1)
    .map((row) => row[promptIdx]?.trim() ?? '')
    .filter((p) => p.length > 0);
}

/**
 * Minimal RFC 4180 CSV row parser.
 * Handles quoted fields, escaped double-quotes (""), and commas within quotes.
 */
function parseCsvRows(content: string): string[][] {
  const rows: string[][] = [];
  let i = 0;
  const len = content.length;

  while (i < len) {
    const { row, nextIndex } = parseRow(content, i, len);
    if (row.length > 0 && !(row.length === 1 && row[0] === '')) {
      rows.push(row);
    }
    i = nextIndex;
  }

  return rows;
}

function parseRow(
  content: string,
  start: number,
  len: number,
): { row: string[]; nextIndex: number } {
  const fields: string[] = [];
  let i = start;

  while (i < len) {
    if (content[i] === '\n' || content[i] === '\r') {
      // End of row — consume \r\n or \n
      if (content[i] === '\r' && i + 1 < len && content[i + 1] === '\n') {
        i += 2;
      } else {
        i += 1;
      }
      break;
    }

    if (content[i] === '"') {
      // Quoted field
      const { value, nextIndex } = parseQuotedField(content, i, len);
      fields.push(value);
      i = nextIndex;
    } else {
      // Unquoted field
      let end = i;
      while (end < len && content[end] !== ',' && content[end] !== '\n' && content[end] !== '\r') {
        end++;
      }
      fields.push(content.slice(i, end));
      i = end;
    }

    // Consume comma separator
    if (i < len && content[i] === ',') {
      i++;
    }
  }

  return { row: fields, nextIndex: i };
}

function parseQuotedField(
  content: string,
  start: number,
  len: number,
): { value: string; nextIndex: number } {
  let i = start + 1; // skip opening quote
  let value = '';

  while (i < len) {
    if (content[i] === '"') {
      if (i + 1 < len && content[i + 1] === '"') {
        // Escaped quote
        value += '"';
        i += 2;
      } else {
        // Closing quote
        i++; // skip closing quote
        break;
      }
    } else {
      value += content[i];
      i++;
    }
  }

  return { value, nextIndex: i };
}
