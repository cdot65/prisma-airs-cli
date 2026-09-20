import pLimit from 'p-limit';
import { buildState, type JudgeUnit } from './ingest.js';
import type { JudgeProvider, RawJudgment, RecordedJudgment } from './providers.js';
import { ProviderError } from './providers.js';
import {
  DEFAULT_SUCCESS_THRESHOLD,
  DEFAULT_UNCERTAIN_BAND,
  MAX_TEXT_CHARS,
  QUESTIONS,
} from './questions.js';

/** Code-owned rule turning Jev's probability into a success verdict. */
export interface SuccessPolicy {
  threshold: number;
  uncertain_low: number;
  uncertain_high: number;
}

export const DEFAULT_POLICY: SuccessPolicy = Object.freeze({
  threshold: DEFAULT_SUCCESS_THRESHOLD,
  uncertain_low: DEFAULT_UNCERTAIN_BAND[0],
  uncertain_high: DEFAULT_UNCERTAIN_BAND[1],
});

/** `[success, uncertain]` for one P(success). */
export function applyPolicy(policy: SuccessPolicy, pSuccess: number): [boolean, boolean] {
  return [
    pSuccess >= policy.threshold,
    policy.uncertain_low <= pSuccess && pSuccess < policy.uncertain_high,
  ];
}

export type JudgmentStatus = 'judged' | 'skipped_error' | 'skipped_oversized' | 'provider_error';

/** One row of judgments.json; key order matches the reference implementation. */
export interface Judgment {
  unit_id: string;
  attack_id: string;
  category: string | null;
  sub_category: string | null;
  severity: string | null;
  status: JudgmentStatus;
  p_success: number | null;
  success: boolean | null;
  uncertain: boolean | null;
  disposition: string | null;
  disposition_confidence: number | null;
  disposition_probabilities: Record<string, number> | null;
  fulfillment_score: number | null;
  fulfillment_confidence: number | null;
  airs_threat: boolean | null;
  airs_asr: number | null;
  marked_safe: boolean | null;
  extraction: string | null;
  model: string | null;
  request_id: string | null;
  latency_ms: number | null;
  input_tokens: number | null;
  error: string | null;
  prompt_sha256: string | null;
  response_sha256: string | null;
}

function baseJudgment(unit: JudgeUnit, status: JudgmentStatus): Judgment {
  return {
    unit_id: unit.unit_id,
    attack_id: unit.attack_id,
    category: unit.category,
    sub_category: unit.sub_category,
    severity: unit.severity,
    status,
    p_success: null,
    success: null,
    uncertain: null,
    disposition: null,
    disposition_confidence: null,
    disposition_probabilities: null,
    fulfillment_score: null,
    fulfillment_confidence: null,
    airs_threat: unit.airs_threat,
    airs_asr: unit.airs_asr,
    marked_safe: unit.marked_safe,
    extraction: unit.extraction,
    model: null,
    request_id: null,
    latency_ms: null,
    input_tokens: null,
    error: null,
    prompt_sha256: unit.prompt_sha256,
    response_sha256: unit.response_sha256,
  };
}

function numberOrNull(value: unknown): number | null {
  return typeof value === 'number' && Number.isFinite(value) ? value : null;
}

function probabilities(value: unknown): Record<string, number> | null {
  if (value === null || typeof value !== 'object' || Array.isArray(value)) return null;
  return Object.fromEntries(
    Object.entries(value as Record<string, unknown>).filter(
      (entry): entry is [string, number] => typeof entry[1] === 'number',
    ),
  );
}

function answer(raw: RawJudgment, id: string): Record<string, unknown> {
  const value = raw.answers[id];
  return value !== null && typeof value === 'object' ? value : {};
}

/** Apply the success policy to a raw provider answer set. */
export function interpret(unit: JudgeUnit, raw: RawJudgment, policy: SuccessPolicy): Judgment {
  const noul = answer(raw, 'objective_achieved');
  const choice = answer(raw, 'response_disposition');
  const score = answer(raw, 'objective_fulfillment');
  const pSuccess = noul.noul;
  if (typeof pSuccess !== 'number' || !(pSuccess >= 0 && pSuccess <= 1))
    throw new ProviderError(
      `objective_achieved answer missing or out of range for unit ${unit.unit_id}`,
    );
  const [success, uncertain] = applyPolicy(policy, pSuccess);
  const usage = raw.usage;
  const inputTokens = usage.input_tokens;
  return {
    ...baseJudgment(unit, 'judged'),
    p_success: pSuccess,
    success,
    uncertain,
    disposition: typeof choice.choice === 'string' ? choice.choice : null,
    disposition_confidence: numberOrNull(choice.confidence),
    disposition_probabilities: probabilities(choice.probabilities),
    fulfillment_score: numberOrNull(score.score),
    fulfillment_confidence: numberOrNull(score.confidence),
    model: raw.model,
    request_id: raw.request_id,
    latency_ms: raw.latency_ms,
    input_tokens: Number.isInteger(inputTokens) ? (inputTokens as number) : null,
  };
}

export interface JudgeUnitsOptions {
  concurrency?: number;
  /** When given, every raw provider answer is stored here keyed by unit id. */
  recorder?: Record<string, RecordedJudgment>;
}

/**
 * Judge every unit with bounded concurrency. Error and empty outputs are skipped without a
 * provider call; provider failures are recorded per unit and never abort the run.
 */
export async function judgeUnits(
  units: JudgeUnit[],
  provider: JudgeProvider,
  policy: SuccessPolicy,
  options: JudgeUnitsOptions = {},
): Promise<Judgment[]> {
  const limit = pLimit(Math.max(1, options.concurrency ?? 4));
  const recorder = options.recorder;
  return Promise.all(
    units.map((unit) =>
      limit(async (): Promise<Judgment> => {
        if (unit.is_error) return baseJudgment(unit, 'skipped_error');
        if (unit.prompt.length > MAX_TEXT_CHARS || unit.response_text.length > MAX_TEXT_CHARS)
          return {
            ...baseJudgment(unit, 'skipped_oversized'),
            error: 'input_exceeds_character_limit',
          };
        try {
          const raw = await provider.judge({
            unitId: unit.unit_id,
            state: buildState(unit),
            questions: QUESTIONS,
          });
          if (recorder)
            recorder[unit.unit_id] = {
              answers: raw.answers,
              model: raw.model,
              usage: raw.usage,
              request_id: raw.request_id,
              prompt_sha256: unit.prompt_sha256,
              response_sha256: unit.response_sha256,
            };
          return interpret(unit, raw, policy);
        } catch (error) {
          if (!(error instanceof ProviderError)) throw error;
          return { ...baseJudgment(unit, 'provider_error'), error: error.message };
        }
      }),
    ),
  );
}
