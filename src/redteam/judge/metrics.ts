import type { IngestionNotes } from './ingest.js';
import type { Judgment, SuccessPolicy } from './judge.js';
import { SCHEMA_VERSION } from './questions.js';

export interface RateBlock {
  judged: number;
  successes: number;
  asr: number | null;
  asr_wilson95: [number, number] | null;
  asr_expected: number | null;
  uncertain: number;
}

export interface AgreementBlock {
  comparable: number;
  both_success: number;
  both_blocked: number;
  judge_only_success: number;
  airs_only_success: number;
  agreement_rate: number | null;
  airs_asr_from_threat_flags: number | null;
}

export interface BreakdownBlock extends RateBlock {
  agreement: AgreementBlock;
}

/** results.json — schema_version 1, identical to the harness skill output. */
export interface JudgeResults {
  schema_version: number;
  provider: string;
  model: string | null;
  policy: SuccessPolicy;
  ingestion: IngestionNotes;
  coverage: {
    units: number;
    judged: number;
    skipped_error: number;
    skipped_oversized?: number;
    provider_error: number;
    attacks_judged: number;
  };
  output_level: RateBlock & { agreement_with_airs: AgreementBlock };
  attack_level: {
    attacks: number;
    successes_any_output: number;
    asr: number | null;
    asr_wilson95: [number, number] | null;
  };
  dispositions: Record<string, number>;
  by_category: Record<string, BreakdownBlock>;
  by_sub_category: Record<string, BreakdownBlock>;
  usage: {
    input_tokens_total: number | null;
    latency_ms_mean: number | null;
  };
}

/** Wilson score interval for a proportion; null when nothing was judged. */
export function wilsonInterval(
  successes: number,
  total: number,
  z = 1.96,
): [number, number] | null {
  if (total <= 0) return null;
  const p = successes / total;
  const denominator = 1 + (z * z) / total;
  const center = (p + (z * z) / (2 * total)) / denominator;
  const half = (z * Math.sqrt((p * (1 - p)) / total + (z * z) / (4 * total * total))) / denominator;
  return [Math.max(0, center - half), Math.min(1, center + half)];
}

function rateBlock(judged: Judgment[]): RateBlock {
  const n = judged.length;
  const successes = judged.filter((j) => j.success).length;
  return {
    judged: n,
    successes,
    asr: n ? successes / n : null,
    asr_wilson95: wilsonInterval(successes, n),
    asr_expected: n ? judged.reduce((sum, j) => sum + (j.p_success ?? 0), 0) / n : null,
    uncertain: judged.filter((j) => j.uncertain).length,
  };
}

function agreement(judged: Judgment[]): AgreementBlock {
  const comparable = judged.filter((j) => j.airs_threat !== null);
  const bothSuccess = comparable.filter((j) => j.success && j.airs_threat).length;
  const bothBlocked = comparable.filter((j) => !j.success && !j.airs_threat).length;
  const judgeOnly = comparable.filter((j) => j.success && !j.airs_threat).length;
  const airsOnly = comparable.filter((j) => !j.success && j.airs_threat).length;
  const n = comparable.length;
  return {
    comparable: n,
    both_success: bothSuccess,
    both_blocked: bothBlocked,
    judge_only_success: judgeOnly,
    airs_only_success: airsOnly,
    agreement_rate: n ? (bothSuccess + bothBlocked) / n : null,
    airs_asr_from_threat_flags: n ? (bothSuccess + airsOnly) / n : null,
  };
}

function groupBy(judged: Judgment[], key: (j: Judgment) => string | null): Map<string, Judgment[]> {
  const groups = new Map<string, Judgment[]>();
  for (const j of judged) {
    const name = key(j) || 'unknown';
    const group = groups.get(name);
    if (group) group.push(j);
    else groups.set(name, [j]);
  }
  return groups;
}

function breakdown(
  judged: Judgment[],
  key: (j: Judgment) => string | null,
): Record<string, BreakdownBlock> {
  const sorted = [...groupBy(judged, key).entries()].sort(([a], [b]) =>
    a < b ? -1 : a > b ? 1 : 0,
  );
  const result: Record<string, BreakdownBlock> = {};
  for (const [name, group] of sorted)
    result[name] = { ...rateBlock(group), agreement: agreement(group) };
  return result;
}

/** Aggregate judgments into the results.json document. */
export function aggregate(
  judgments: Judgment[],
  policy: SuccessPolicy,
  meta: { providerName: string; model: string | null; ingestion: IngestionNotes },
): JudgeResults {
  const judged = judgments.filter((j) => j.status === 'judged');
  const byAttack = groupBy(judged, (j) => j.attack_id);
  const attackSuccesses = [...byAttack.values()].filter((group) =>
    group.some((j) => j.success),
  ).length;
  const dispositions: Record<string, number> = {};
  for (const j of judged) {
    const name = j.disposition || 'unknown';
    dispositions[name] = (dispositions[name] ?? 0) + 1;
  }
  const tokens = judged
    .map((j) => j.input_tokens)
    .filter((value): value is number => Number.isInteger(value));
  const latencies = judged
    .map((j) => j.latency_ms)
    .filter((value): value is number => typeof value === 'number');
  return {
    schema_version: SCHEMA_VERSION,
    provider: meta.providerName,
    model: meta.model,
    policy: {
      threshold: policy.threshold,
      uncertain_low: policy.uncertain_low,
      uncertain_high: policy.uncertain_high,
    },
    ingestion: meta.ingestion,
    coverage: {
      units: judgments.length,
      judged: judged.length,
      skipped_error: judgments.filter((j) => j.status === 'skipped_error').length,
      provider_error: judgments.filter((j) => j.status === 'provider_error').length,
      attacks_judged: byAttack.size,
      ...(judgments.some((j) => j.status === 'skipped_oversized')
        ? { skipped_oversized: judgments.filter((j) => j.status === 'skipped_oversized').length }
        : {}),
    },
    output_level: { ...rateBlock(judged), agreement_with_airs: agreement(judged) },
    attack_level: {
      attacks: byAttack.size,
      successes_any_output: attackSuccesses,
      asr: byAttack.size ? attackSuccesses / byAttack.size : null,
      asr_wilson95: wilsonInterval(attackSuccesses, byAttack.size),
    },
    dispositions,
    by_category: breakdown(judged, (j) => j.category),
    by_sub_category: breakdown(judged, (j) => j.sub_category),
    usage: {
      input_tokens_total: tokens.length ? tokens.reduce((sum, t) => sum + t, 0) : null,
      latency_ms_mean: latencies.length
        ? latencies.reduce((sum, l) => sum + l, 0) / latencies.length
        : null,
    },
  };
}
