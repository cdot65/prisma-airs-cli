import { createHash } from 'node:crypto';
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

function textFromContentList(content: unknown): string | null {
  if (!Array.isArray(content)) return null;
  const parts = content
    .filter((c): c is Dict => isDict(c) && typeof c.text === 'string')
    .map((c) => c.text as string);
  return parts.length ? parts.join('\n') : null;
}

/**
 * Return `[text, extractionMethod]` for a raw AIRS `output` value. Handles plain text,
 * stringified JSON in OpenAI Responses or Chat Completions shape, multi-turn objects,
 * and falls back to the raw string.
 */
export function extractResponseText(raw: unknown): [string, string] {
  if (raw === null || raw === undefined) return ['', 'empty'];
  let value: unknown = raw;
  let method = 'plain';
  if (typeof raw === 'string') {
    const stripped = raw.trim();
    if (!stripped) return ['', 'empty'];
    if (stripped[0] !== '{' && stripped[0] !== '[') return [raw, 'plain'];
    try {
      value = JSON.parse(stripped);
      method = 'json';
    } catch {
      return [raw, 'plain'];
    }
  }
  if (isDict(value)) {
    const outputs = value.output;
    if (Array.isArray(outputs)) {
      const texts = outputs
        .map((item) => (isDict(item) ? textFromContentList(item.content) : null))
        .filter((text): text is string => Boolean(text));
      if (texts.length) return [texts.join('\n'), 'responses_api'];
    }
    const choices = value.choices;
    if (Array.isArray(choices) && choices.length && isDict(choices[0])) {
      const message = choices[0].message;
      if (isDict(message) && typeof message.content === 'string')
        return [message.content, 'chat_completions'];
    }
    for (const key of ['output_text', 'response', 'text', 'content', 'message', 'answer']) {
      if (typeof value[key] === 'string') return [value[key] as string, `json.${key}`];
    }
    const turns = value.messages || value.turns || value.conversation;
    if (Array.isArray(turns) && turns.length) {
      const last = turns[turns.length - 1];
      if (isDict(last)) {
        const text =
          typeof last.content === 'string' ? last.content : textFromContentList(last.content);
        if (text) return [text, 'multi_turn_last'];
      }
    }
    return [pythonJson(value), `${method}_unparsed`];
  }
  if (Array.isArray(value)) {
    const texts = value.filter((item): item is string => typeof item === 'string');
    if (texts.length) return [texts.join('\n'), 'list_of_strings'];
    return [pythonJson(value), `${method}_unparsed`];
  }
  return [String(value), 'coerced'];
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
  const notes: IngestionNotes = {
    layout,
    records: records.length,
    skipped_no_prompt: 0,
    error_outputs: 0,
  };
  const limit = options.limit;
  for (const [index, record] of records.entries()) {
    const prompt = record.prompt;
    if (typeof prompt !== 'string' || !prompt.trim()) {
      notes.skipped_no_prompt += 1;
      continue;
    }
    const attackId =
      label(record.uuid) || label(record.attack_id) || label(record.id) || `index-${index}`;
    const subCategory = label(record.sub_category_display_name) || label(record.sub_category);
    const objective =
      label(record.goal) ||
      `${label(record.category_display_name) || label(record.category) || 'unknown category'} / ${subCategory || 'unknown sub-category'} attack: make the target do what the prompt asks.`;
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
      units.push({
        unit_id: `${attackId}#${outputId}`,
        attack_id: attackId,
        job_id: label(record.job_id) || label(output.job_id),
        category: label(record.category),
        sub_category: subCategory,
        severity: label(record.severity),
        objective,
        prompt,
        response_text: text.slice(0, MAX_TEXT_CHARS),
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
  state.attack.prompt = `<${unit.prompt.length} chars, sha256 ${unit.prompt_sha256}>`;
  state.target_response = `<${unit.response_text.length} chars, sha256 ${unit.response_sha256}>`;
  return state;
}
