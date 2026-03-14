import chalk from 'chalk';
import type {
  ModelSecurityEvaluation,
  ModelSecurityFile,
  ModelSecurityGroup,
  ModelSecurityRule,
  ModelSecurityRuleInstance,
  ModelSecurityScan,
  ModelSecurityViolation,
} from '../../airs/types.js';
import { formatOutput, type OutputFormat } from './common.js';

/** Render the model security banner. */
export function renderModelSecurityHeader(): void {
  console.log(chalk.bold.blue('\n  Prisma AIRS — Model Security'));
  console.log(chalk.dim('  ML model supply chain security\n'));
}

/** Render security group list. */
export function renderGroupList(
  groups: ModelSecurityGroup[],
  format: OutputFormat = 'pretty',
): void {
  if (groups.length === 0) {
    console.log(chalk.dim('  No security groups found.\n'));
    return;
  }
  if (format !== 'pretty') {
    const rows = groups.map((g) => ({
      id: g.uuid,
      name: g.name,
      state: g.state,
      sourceType: g.sourceType,
    }));
    console.log(
      formatOutput(
        rows,
        [
          { key: 'id', label: 'ID' },
          { key: 'name', label: 'Name' },
          { key: 'state', label: 'State' },
          { key: 'sourceType', label: 'Source Type' },
        ],
        format,
      ),
    );
    return;
  }
  console.log(chalk.bold('\n  Security Groups:\n'));
  for (const g of groups) {
    console.log(`  ${chalk.dim(g.uuid)}`);
    const stateColor = g.state === 'ACTIVE' ? chalk.green : chalk.yellow;
    console.log(`    ${g.name}  ${stateColor(g.state)}  source: ${chalk.dim(g.sourceType)}`);
  }
  console.log();
}

/** Render security group detail. */
export function renderGroupDetail(group: ModelSecurityGroup): void {
  console.log(chalk.bold('\n  Security Group Detail:\n'));
  console.log(`    UUID:        ${chalk.dim(group.uuid)}`);
  console.log(`    Name:        ${group.name}`);
  console.log(`    Description: ${group.description || chalk.dim('(none)')}`);
  console.log(`    Source Type: ${group.sourceType}`);
  const stateColor = group.state === 'ACTIVE' ? chalk.green : chalk.yellow;
  console.log(`    State:       ${stateColor(group.state)}`);
  console.log(`    Created:     ${chalk.dim(group.createdAt)}`);
  console.log(`    Updated:     ${chalk.dim(group.updatedAt)}`);
  console.log();
}

/** Render security rule list. */
export function renderRuleList(rules: ModelSecurityRule[], format: OutputFormat = 'pretty'): void {
  if (rules.length === 0) {
    console.log(chalk.dim('  No security rules found.\n'));
    return;
  }
  if (format !== 'pretty') {
    const rows = rules.map((r) => ({
      id: r.uuid,
      name: r.name,
      type: r.ruleType,
      defaultState: r.defaultState,
      sources: r.compatibleSources.join(', '),
    }));
    console.log(
      formatOutput(
        rows,
        [
          { key: 'id', label: 'ID' },
          { key: 'name', label: 'Name' },
          { key: 'type', label: 'Type' },
          { key: 'defaultState', label: 'Default State' },
          { key: 'sources', label: 'Sources' },
        ],
        format,
      ),
    );
    return;
  }
  console.log(chalk.bold('\n  Security Rules:\n'));
  for (const r of rules) {
    console.log(`  ${chalk.dim(r.uuid)}`);
    console.log(
      `    ${r.name}  type: ${chalk.dim(r.ruleType)}  default: ${chalk.dim(r.defaultState)}`,
    );
    console.log(`    ${chalk.dim(r.description)}`);
    console.log(`    Sources: ${r.compatibleSources.map((s) => chalk.dim(s)).join(', ')}`);
  }
  console.log();
}

/** Render security rule detail. */
export function renderRuleDetail(rule: ModelSecurityRule): void {
  console.log(chalk.bold('\n  Security Rule Detail:\n'));
  console.log(`    UUID:          ${chalk.dim(rule.uuid)}`);
  console.log(`    Name:          ${rule.name}`);
  console.log(`    Description:   ${rule.description}`);
  console.log(`    Rule Type:     ${rule.ruleType}`);
  console.log(`    Default State: ${rule.defaultState}`);
  console.log(`    Sources:       ${rule.compatibleSources.join(', ')}`);

  if (rule.remediation.description) {
    console.log(chalk.bold('\n    Remediation:'));
    console.log(`      ${rule.remediation.description}`);
    if (rule.remediation.steps.length > 0) {
      for (const step of rule.remediation.steps) {
        console.log(`      ${chalk.dim('•')} ${step}`);
      }
    }
    if (rule.remediation.url) {
      console.log(`      ${chalk.dim(rule.remediation.url)}`);
    }
  }

  if (rule.editableFields.length > 0) {
    console.log(chalk.bold('\n    Editable Fields:'));
    for (const f of rule.editableFields) {
      console.log(`      ${f.displayName} (${chalk.dim(f.attributeName)}): ${f.displayType}`);
      if (f.description) console.log(`        ${chalk.dim(f.description)}`);
    }
  }
  console.log();
}

/** Render rule instance list. */
export function renderRuleInstanceList(instances: ModelSecurityRuleInstance[]): void {
  if (instances.length === 0) {
    console.log(chalk.dim('  No rule instances found.\n'));
    return;
  }
  console.log(chalk.bold('\n  Rule Instances:\n'));
  for (const ri of instances) {
    const stateColor =
      ri.state === 'BLOCKING' ? chalk.red : ri.state === 'ALLOWING' ? chalk.green : chalk.dim;
    const ruleName = (ri.rule as { name?: string })?.name ?? ri.securityRuleUuid;
    console.log(`  ${chalk.dim(ri.uuid)}`);
    console.log(`    ${ruleName}  ${stateColor(ri.state)}`);
  }
  console.log();
}

/** Render rule instance detail. */
export function renderRuleInstanceDetail(instance: ModelSecurityRuleInstance): void {
  console.log(chalk.bold('\n  Rule Instance Detail:\n'));
  console.log(`    UUID:         ${chalk.dim(instance.uuid)}`);
  console.log(`    Group UUID:   ${chalk.dim(instance.securityGroupUuid)}`);
  console.log(`    Rule UUID:    ${chalk.dim(instance.securityRuleUuid)}`);
  const stateColor =
    instance.state === 'BLOCKING'
      ? chalk.red
      : instance.state === 'ALLOWING'
        ? chalk.green
        : chalk.dim;
  console.log(`    State:        ${stateColor(instance.state)}`);
  const ruleName = (instance.rule as { name?: string })?.name;
  if (ruleName) console.log(`    Rule Name:    ${ruleName}`);
  console.log(`    Created:      ${chalk.dim(instance.createdAt)}`);
  console.log(`    Updated:      ${chalk.dim(instance.updatedAt)}`);

  if (Object.keys(instance.fieldValues).length > 0) {
    console.log(chalk.bold('\n    Field Values:'));
    for (const [key, value] of Object.entries(instance.fieldValues)) {
      const display = Array.isArray(value) ? value.join(', ') : String(value);
      console.log(`      ${key}: ${chalk.dim(display)}`);
    }
  }
  console.log();
}

// ---------------------------------------------------------------------------
// Model Security — Scans
// ---------------------------------------------------------------------------

/** Render a list of model security scans. */
export function renderMsScanList(
  scans: ModelSecurityScan[],
  format: OutputFormat = 'pretty',
): void {
  if (scans.length === 0) {
    console.log(chalk.dim('  No scans found.\n'));
    return;
  }
  if (format !== 'pretty') {
    const rows = scans.map((s) => ({
      id: s.uuid,
      outcome: s.evalOutcome,
      origin: s.scanOrigin,
      modelUri: s.modelUri ?? '',
      createdAt: s.createdAt,
      passed: s.evalSummary?.rulesPassed ?? '',
      failed: s.evalSummary?.rulesFailed ?? '',
    }));
    console.log(
      formatOutput(
        rows,
        [
          { key: 'id', label: 'ID' },
          { key: 'outcome', label: 'Outcome' },
          { key: 'origin', label: 'Origin' },
          { key: 'modelUri', label: 'Model URI' },
          { key: 'passed', label: 'Passed' },
          { key: 'failed', label: 'Failed' },
          { key: 'createdAt', label: 'Created' },
        ],
        format,
      ),
    );
    return;
  }
  console.log(chalk.bold('\n  Model Security Scans:\n'));
  for (const s of scans) {
    const outcomeColor =
      s.evalOutcome === 'ALLOWED'
        ? chalk.green
        : s.evalOutcome === 'BLOCKED'
          ? chalk.red
          : chalk.yellow;
    console.log(`  ${chalk.dim(s.uuid)}`);
    console.log(
      `    ${outcomeColor(s.evalOutcome)}  ${chalk.dim(s.scanOrigin)}  ${chalk.dim(s.createdAt)}`,
    );
    if (s.modelUri) console.log(`    ${chalk.dim(s.modelUri)}`);
    if (s.evalSummary) {
      const { rulesPassed, rulesFailed, totalRules } = s.evalSummary;
      console.log(
        `    Rules: ${chalk.green(`${rulesPassed} passed`)}  ${chalk.red(`${rulesFailed} failed`)}  / ${totalRules} total`,
      );
    }
  }
  console.log();
}

/** Render full scan detail. */
export function renderMsScanDetail(scan: ModelSecurityScan): void {
  console.log(chalk.bold('\n  Scan Detail:\n'));
  console.log(`    UUID:       ${chalk.dim(scan.uuid)}`);
  const outcomeColor =
    scan.evalOutcome === 'ALLOWED'
      ? chalk.green
      : scan.evalOutcome === 'BLOCKED'
        ? chalk.red
        : chalk.yellow;
  console.log(`    Outcome:    ${outcomeColor(scan.evalOutcome)}`);
  if (scan.modelUri) console.log(`    Model URI:  ${scan.modelUri}`);
  console.log(`    Origin:     ${scan.scanOrigin}`);
  console.log(`    Source:     ${scan.sourceType}`);
  console.log(`    Group:      ${scan.securityGroupName}`);
  console.log(`    Created:    ${chalk.dim(scan.createdAt)}`);
  console.log(`    Updated:    ${chalk.dim(scan.updatedAt)}`);
  if (scan.evalSummary) {
    const { rulesPassed, rulesFailed, totalRules } = scan.evalSummary;
    console.log(
      `    Rules:      ${chalk.green(`${rulesPassed} passed`)}  ${chalk.red(`${rulesFailed} failed`)}  / ${totalRules} total`,
    );
  }
  if (scan.labels.length > 0) {
    console.log(chalk.bold('\n    Labels:'));
    for (const l of scan.labels) {
      console.log(`      ${l.key}: ${chalk.dim(l.value)}`);
    }
  }
  console.log();
}

// ---------------------------------------------------------------------------
// Model Security — Evaluations
// ---------------------------------------------------------------------------

/** Render a list of evaluations. */
export function renderEvaluationList(evaluations: ModelSecurityEvaluation[]): void {
  if (evaluations.length === 0) {
    console.log(chalk.dim('  No evaluations found.\n'));
    return;
  }
  console.log(chalk.bold('\n  Rule Evaluations:\n'));
  for (const e of evaluations) {
    const color =
      e.result === 'PASSED' ? chalk.green : e.result === 'FAILED' ? chalk.red : chalk.yellow;
    console.log(`  ${chalk.dim(e.uuid)}`);
    console.log(`    ${e.ruleName}  ${color(e.result)}  ${chalk.dim(e.ruleInstanceState)}`);
  }
  console.log();
}

/** Render a single evaluation detail. */
export function renderEvaluationDetail(evaluation: ModelSecurityEvaluation): void {
  console.log(chalk.bold('\n  Evaluation Detail:\n'));
  console.log(`    UUID:           ${chalk.dim(evaluation.uuid)}`);
  console.log(`    Rule:           ${evaluation.ruleName}`);
  console.log(`    Description:    ${chalk.dim(evaluation.ruleDescription)}`);
  console.log(`    Instance UUID:  ${chalk.dim(evaluation.ruleInstanceUuid)}`);
  console.log(`    Instance State: ${evaluation.ruleInstanceState}`);
  const color =
    evaluation.result === 'PASSED'
      ? chalk.green
      : evaluation.result === 'FAILED'
        ? chalk.red
        : chalk.yellow;
  console.log(`    Result:         ${color(evaluation.result)}`);
  console.log(`    Violations:     ${evaluation.violationCount}`);
  console.log();
}

// ---------------------------------------------------------------------------
// Model Security — Violations
// ---------------------------------------------------------------------------

/** Render a list of violations. */
export function renderViolationList(violations: ModelSecurityViolation[]): void {
  if (violations.length === 0) {
    console.log(chalk.dim('  No violations found.\n'));
    return;
  }
  console.log(chalk.bold('\n  Violations:\n'));
  for (const v of violations) {
    console.log(`  ${chalk.dim(v.uuid)}`);
    console.log(`    ${chalk.red(v.ruleName)}  ${chalk.dim(v.file)}`);
    console.log(`    ${v.description}`);
    console.log(`    Threat: ${chalk.dim(v.threat)}`);
  }
  console.log();
}

/** Render a single violation detail. */
export function renderViolationDetail(violation: ModelSecurityViolation): void {
  console.log(chalk.bold('\n  Violation Detail:\n'));
  console.log(`    UUID:        ${chalk.dim(violation.uuid)}`);
  console.log(`    Rule:        ${chalk.red(violation.ruleName)}`);
  console.log(`    Description: ${violation.ruleDescription}`);
  console.log(`    State:       ${violation.ruleInstanceState}`);
  console.log(`    File:        ${violation.file}`);
  console.log(`    Threat:      ${violation.threat}`);
  console.log(`    Detail:      ${violation.description}`);
  console.log();
}

// ---------------------------------------------------------------------------
// Model Security — Files
// ---------------------------------------------------------------------------

/** Render a list of scanned files. */
export function renderFileList(files: ModelSecurityFile[]): void {
  if (files.length === 0) {
    console.log(chalk.dim('  No files found.\n'));
    return;
  }
  console.log(chalk.bold('\n  Scanned Files:\n'));
  for (const f of files) {
    const color =
      f.result === 'SUCCESS' ? chalk.green : f.result === 'SKIPPED' ? chalk.yellow : chalk.red;
    const formats = f.formats.length > 0 ? chalk.dim(` [${f.formats.join(', ')}]`) : '';
    console.log(`    ${color(f.result)}  ${f.type}  ${f.path}${formats}`);
  }
  console.log();
}

// ---------------------------------------------------------------------------
// Model Security — Labels
// ---------------------------------------------------------------------------

/** Render label keys. */
export function renderLabelKeys(keys: string[]): void {
  if (keys.length === 0) {
    console.log(chalk.dim('  No label keys found.\n'));
    return;
  }
  console.log(chalk.bold('\n  Label Keys:\n'));
  for (const k of keys) {
    console.log(`    ${k}`);
  }
  console.log();
}

/** Render label values for a key. */
export function renderLabelValues(key: string, values: string[]): void {
  if (values.length === 0) {
    console.log(chalk.dim(`  No values for key "${key}".\n`));
    return;
  }
  console.log(chalk.bold(`\n  Label Values for "${key}":\n`));
  for (const v of values) {
    console.log(`    ${v}`);
  }
  console.log();
}
