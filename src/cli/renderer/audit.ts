import chalk from 'chalk';
import type { AuditResult, ConflictPair, ProfileTopic } from '../../audit/types.js';

/** Render profile topics discovered during audit. */
export function renderAuditTopics(topics: ProfileTopic[]): void {
  for (const t of topics) {
    const actionColor = t.action === 'block' ? chalk.red : chalk.green;
    console.log(`  ${actionColor(`[${t.action}]`)} ${chalk.bold(t.topicName)}`);
    if (t.description) console.log(chalk.dim(`    ${t.description}`));
  }
  console.log();
}

/** Render audit completion with per-topic metrics table. */
export function renderAuditComplete(result: AuditResult): void {
  console.log(chalk.bold('\n  Per-Topic Results:\n'));
  console.log(
    chalk.dim('  Topic                          Coverage  TPR     TNR     Accuracy  Tests'),
  );
  console.log(chalk.dim(`  ${'─'.repeat(72)}`));
  for (const tr of result.topics) {
    const m = tr.metrics;
    const name = tr.topic.topicName.padEnd(30);
    const cov = `${(m.coverage * 100).toFixed(0)}%`.padStart(6);
    const tpr = `${(m.truePositiveRate * 100).toFixed(0)}%`.padStart(6);
    const tnr = `${(m.trueNegativeRate * 100).toFixed(0)}%`.padStart(6);
    const acc = `${(m.accuracy * 100).toFixed(0)}%`.padStart(6);
    const tests = String(tr.testResults.length).padStart(5);
    const covColor = m.coverage >= 0.9 ? chalk.green : m.coverage >= 0.5 ? chalk.yellow : chalk.red;
    console.log(`  ${name}  ${covColor(cov)}  ${tpr}  ${tnr}  ${acc}  ${tests}`);
  }
}

/** Render detected cross-topic conflicts. */
export function renderConflicts(conflicts: ConflictPair[]): void {
  console.log(chalk.bold.yellow(`\n  Conflicts Detected: ${conflicts.length}\n`));
  for (const c of conflicts) {
    console.log(chalk.yellow(`  ${c.topicA} ↔ ${c.topicB}`));
    console.log(chalk.dim(`    ${c.description}`));
    for (const e of c.evidence.slice(0, 3)) {
      console.log(chalk.dim(`    • "${e}"`));
    }
    if (c.evidence.length > 3) {
      console.log(chalk.dim(`    ...and ${c.evidence.length - 3} more`));
    }
  }
  console.log();
}
