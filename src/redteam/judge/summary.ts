import type { JudgeResults } from './metrics.js';

function pct(value: number | null): string {
  return value === null ? 'n/a' : `${(100 * value).toFixed(1)}%`;
}

/** summary.md — the same rows as the harness skill so the two are interchangeable. */
export function renderSummary(results: JudgeResults): string {
  const out = results.output_level;
  const attack = results.attack_level;
  const agreement = out.agreement_with_airs;
  const interval = out.asr_wilson95;
  const lines = [
    '# Red-team ASR judgment summary',
    '',
    `Provider: ${results.provider} (model ${results.model}). Success threshold ${results.policy.threshold}.`,
    '',
    `Objective proxies: ${results.ingestion.objective_proxies ?? 0} records. Source-row IDs substituted: ${results.ingestion.fallback_row_ids ?? 0} records. Grouped ASR uses row identity where source attack IDs are absent.`,
    '',
    '| Metric | Value |',
    '|---|---|',
    `| Units judged / total | ${results.coverage.judged} / ${results.coverage.units} |`,
    `| Skipped (error or empty output) | ${results.coverage.skipped_error} |`,
    `| Skipped (oversized input) | ${results.coverage.skipped_oversized ?? 0} |`,
    `| Provider errors | ${results.coverage.provider_error} |`,
    `| Output-level ASR (threshold) | ${pct(out.asr)} |`,
    `| Output-level ASR 95% Wilson interval | ${interval ? `${pct(interval[0])} to ${pct(interval[1])}` : 'n/a'} |`,
    `| Output-level expected ASR (mean P(success)) | ${pct(out.asr_expected)} |`,
    `| Uncertain judgments (review band) | ${out.uncertain} |`,
    `| Attack-level ASR (any output succeeded) | ${pct(attack.asr)} |`,
    `| AIRS ASR from threat flags | ${pct(agreement.airs_asr_from_threat_flags)} |`,
    `| Agreement with AIRS | ${pct(agreement.agreement_rate)} |`,
    `| Judge-only successes / AIRS-only successes | ${agreement.judge_only_success} / ${agreement.airs_only_success} |`,
    '',
    '## By category',
    '',
    '| Category | Judged | Judge ASR | AIRS ASR | Agreement |',
    '|---|---|---|---|---|',
  ];
  for (const [name, block] of Object.entries(results.by_category))
    lines.push(
      `| ${name} | ${block.judged} | ${pct(block.asr)} | ${pct(block.agreement.airs_asr_from_threat_flags)} | ${pct(block.agreement.agreement_rate)} |`,
    );
  lines.push('', '## Dispositions', '');
  for (const [name, count] of Object.entries(results.dispositions).sort(([a], [b]) =>
    a < b ? -1 : a > b ? 1 : 0,
  ))
    lines.push(`- ${name}: ${count}`);
  lines.push(
    '',
    'Judge verdicts are model outputs under a stated policy, not validated ground truth. See references/methodology.md.',
  );
  return `${lines.join('\n')}\n`;
}
