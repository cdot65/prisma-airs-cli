import { createHash } from 'node:crypto';
import { decodeContainer, messageText } from './envelope.js';
import { MAX_TEXT_CHARS } from './questions.js';

/** One (attack, output) pair to judge, with the AIRS metadata kept alongside. */
export interface JudgeUnit {
  unit_id: string;
  attack_id: string;
  job_id: string | null;
  category: string | null;
  sub_category: string | null;
  severity: string | null;
  objective: string;
  prompt: string;
  response_text: string;
  airs_threat: boolean | null;
  airs_asr: number | null;
  marked_safe: boolean | null;
  is_error: boolean;
  extraction: string;
  prompt_sha256: string;
  response_sha256: string;
}

/** Ingestion notes reported in results.json (`ingestion`). */
export interface IngestionNotes {
  layout: string;
  records: number;
  skipped_no_prompt: number;
  error_outputs: number;
  truncated_to_limit?: number;
  fallback_row_ids?: number;
  objective_proxies?: number;
  oversized_units?: number;
  normalized_prompt_envelopes?: number;
}

/** The state object sent to Jev for one unit. */
export interface JudgeState {
  attack: {
    category: string | null;
    sub_category: string | null;
    objective: string;
    prompt: string;
  };
  target_response: string;
}

type Dict = Record<string, unknown>;

function isDict(value: unknown): value is Dict {
  return value !== null && typeof value === 'object' && !Array.isArray(value);
}

export function digest(text: string): string {
  return createHash('sha256').update(text, 'utf8').digest('hex').slice(0, 16);
}

/**
 * Serialize like Python's `json.dumps(value, ensure_ascii=False)` (", " and ": "
 * separators) so re-serialized fallbacks hash identically across both implementations.
 */
export function pythonJson(value: unknown): string {
  if (Array.isArray(value)) return `[${value.map(pythonJson).join(', ')}]`;
  if (isDict(value))
    return `{${Object.entries(value)
      .filter(([, v]) => v !== undefined)
      .map(([k, v]) => `${JSON.stringify(k)}: ${pythonJson(v)}`)
      .join(', ')}}`;
  return JSON.stringify(value ?? null);
}

/** Preserve the model output string verbatim, including serialized JSON and message envelopes. */
export function extractResponseText(raw: unknown): [string, string] {
  if (raw === null || raw === undefined) return ['', 'empty'];
  if (typeof raw !== 'string') throw new Error('Expected output to be a string');
  return [raw, raw.trim() ? 'plain' : 'empty'];
}

function recordsFromDocument(document: unknown): [Dict[], string] {
  if (Array.isArray(document)) return [document.filter(isDict), 'array'];
  if (isDict(document)) {
    for (const key of ['attacks', 'data', 'results', 'records', 'items']) {
      const list = document[key];
      if (Array.isArray(list)) return [list.filter(isDict), `object.${key}`];
    }
    if ('prompt' in document && ('output' in document || 'outputs' in document))
      return [[document], 'single'];
  }
  throw new Error(
    'Unrecognized scan JSON: expected an array of attack records or an object with an attacks/data array',
  );
}

function boolOrNull(value: unknown): boolean | null {
  if (typeof value === 'boolean') return value;
  if (typeof value === 'string' && ['true', 'false'].includes(value.toLowerCase()))
    return value.toLowerCase() === 'true';
  return null;
}

function label(value: unknown): string | null {
  return typeof value === 'string' && value ? value : null;
}

/** Flatten a scan document into judge units and report ingestion notes. */
export function normalizeScan(
  document: unknown,
  options: { limit?: number } = {},
): { units: JudgeUnit[]; notes: IngestionNotes } {
  const [records, layout] = recordsFromDocument(document);
  const units: JudgeUnit[] = [];
  const unitIds = new Set<string>();
  const notes: IngestionNotes = {
    layout,
    records: records.length,
    skipped_no_prompt: 0,
    error_outputs: 0,
  };
  const limit = options.limit;
  for (const [index, record] of records.entries()) {
    const rawPrompt = record.prompt;
    if (typeof rawPrompt !== 'string' || !rawPrompt.trim()) {
      notes.skipped_no_prompt += 1;
      continue;
    }
    let prompt: string = rawPrompt;
    // Only unwrap explicit envelopes; arbitrary JSON prompts are attack content.
    const promptMessage = prompt.trimStart().startsWith('{')
      ? messageText(decodeContainer(prompt.trim())[0])
      : null;
    if (promptMessage !== null) {
      prompt = promptMessage[0];
      if (!prompt.trim()) {
        notes.skipped_no_prompt += 1;
        continue;
      }
      notes.normalized_prompt_envelopes = (notes.normalized_prompt_envelopes ?? 0) + 1;
    }
    const attackId =
      label(record.uuid) || label(record.attack_id) || label(record.id) || `index-${index}`;
    if (attackId === `index-${index}`) notes.fallback_row_ids = (notes.fallback_row_ids ?? 0) + 1;
    if (!label(record.goal)) notes.objective_proxies = (notes.objective_proxies ?? 0) + 1;
    const subCategory = label(record.sub_category_display_name) || label(record.sub_category);
    const objective =
      label(record.goal) ||
      `${label(record.category_display_name) || label(record.category) || 'unknown category'} / ${label(record.goal_category) || subCategory || 'unknown sub-category'} attack: make the target do what the prompt asks.`;
    const rawOutputs: Dict[] = Array.isArray(record.outputs)
      ? record.outputs.filter(isDict)
      : [
          {
            output: record.output,
            threat: record.threat,
            error: record.error,
            error_message: record.error_message,
            marked_safe: record.marked_safe,
          },
        ];
    const airsAsr = typeof record.asr === 'number' ? record.asr : null;
    for (const [outputIndex, output] of rawOutputs.entries()) {
      const [text, extraction] = extractResponseText(output.output);
      const isError = Boolean(output.error) || !text.trim();
      if (isError) notes.error_outputs += 1;
      const outputId = label(output.uuid) || String(outputIndex);
      const unitId = `${attackId}#${outputId}`;
      if (unitIds.has(unitId)) throw new Error('Scan contains duplicate attack/output identifiers');
      unitIds.add(unitId);
      if (text.length > MAX_TEXT_CHARS || prompt.length > MAX_TEXT_CHARS)
        notes.oversized_units = (notes.oversized_units ?? 0) + 1;
      units.push({
        unit_id: `${attackId}#${outputId}`,
        attack_id: attackId,
        job_id: label(record.job_id) || label(output.job_id),
        category: label(record.category),
        sub_category: subCategory,
        severity: label(record.severity),
        objective,
        prompt,
        response_text: text,
        airs_threat: boolOrNull('threat' in output ? output.threat : record.threat),
        airs_asr: airsAsr,
        marked_safe: boolOrNull('marked_safe' in output ? output.marked_safe : record.marked_safe),
        is_error: isError,
        extraction,
        prompt_sha256: digest(prompt),
        response_sha256: digest(text),
      });
      if (limit !== undefined && units.length >= limit) {
        notes.truncated_to_limit = limit;
        return { units, notes };
      }
    }
  }
  return { units, notes };
}

export function buildState(unit: JudgeUnit): JudgeState {
  return {
    attack: {
      category: unit.category,
      sub_category: unit.sub_category,
      objective: unit.objective,
      prompt: unit.prompt,
    },
    target_response: unit.response_text,
  };
}

/** The dry-run view of a state: lengths and hashes instead of prompt and response text. */
export function redactState(unit: JudgeUnit): JudgeState {
  const state = buildState(unit);
  state.attack.objective = `<redacted, sha256 ${digest(unit.objective)}>`;
  state.attack.prompt = `<${unit.prompt.length} chars, sha256 ${unit.prompt_sha256}>`;
  state.target_response = `<${unit.response_text.length} chars, sha256 ${unit.response_sha256}>`;
  return state;
}
