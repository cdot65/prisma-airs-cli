import chalk from 'chalk';
import { dump as yamlDump } from 'js-yaml';
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
import { ui } from './ui.js';

/** Render the model security banner. */
export function renderModelSecurityHeader(): void {
  ui.header('Prisma AIRS — Model Security', 'ML model supply chain security');
}

/** State/result → inline color (value coloring within composed lines). */
function stateColor(state: string): (s: string) => string {
  switch (state) {
    case 'ACTIVE':
    case 'ALLOWED':
    case 'ALLOWING':
    case 'PASSED':
    case 'SUCCESS':
      return chalk.green;
    case 'BLOCKED':
    case 'BLOCKING':
    case 'FAILED':
      return chalk.red;
    case 'DISABLED':
      return chalk.dim;
    default:
      return chalk.yellow;
  }
}

/** Render security group list. */
export function renderGroupList(
  groups: ModelSecurityGroup[],
  format: OutputFormat = 'pretty',
): void {
  if (groups.length === 0) {
    ui.emptyList('security groups');
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
  ui.section('Security Groups:');
  for (const g of groups) {
    ui.dim(g.uuid);
    const color = g.state === 'ACTIVE' ? chalk.green : chalk.yellow;
    console.log(`    ${g.name}  ${color(g.state)}  source: ${chalk.dim(g.sourceType)}`);
  }
  console.log();
}

/** Render security group detail. */
export function renderGroupDetail(
  group: ModelSecurityGroup,
  format: OutputFormat = 'pretty',
): void {
  if (format === 'json') {
    console.log(JSON.stringify(group, null, 2));
    return;
  }
  if (format === 'yaml') {
    console.log(yamlDump(group));
    return;
  }
  ui.section('Security Group Detail:');
  const color = group.state === 'ACTIVE' ? chalk.green : chalk.yellow;
  ui.keyValue([
    ['UUID', group.uuid],
    ['Name', group.name],
    ['Description', group.description || chalk.dim('(none)')],
    ['Source Type', group.sourceType],
    ['State', color(group.state)],
    ['Created', group.createdAt],
    ['Updated', group.updatedAt],
  ]);
  console.log();
}

/** Render security rule list. */
export function renderRuleList(rules: ModelSecurityRule[], format: OutputFormat = 'pretty'): void {
  if (rules.length === 0) {
    ui.emptyList('security rules');
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
  ui.section('Security Rules:');
  for (const r of rules) {
    ui.dim(r.uuid);
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
  ui.section('Security Rule Detail:');
  ui.keyValue([
    ['UUID', rule.uuid],
    ['Name', rule.name],
    ['Description', rule.description],
    ['Rule Type', rule.ruleType],
    ['Default State', rule.defaultState],
    ['Sources', rule.compatibleSources.join(', ')],
  ]);

  if (rule.remediation.description) {
    ui.section('Remediation:');
    console.log(`  ${rule.remediation.description}`);
    if (rule.remediation.steps.length > 0) {
      for (const step of rule.remediation.steps) {
        ui.bullet(step, 'neutral');
      }
    }
    if (rule.remediation.url) {
      ui.dim(rule.remediation.url);
    }
  }

  if (rule.editableFields.length > 0) {
    ui.section('Editable Fields:');
    for (const f of rule.editableFields) {
      console.log(`  ${f.displayName} (${chalk.dim(f.attributeName)}): ${f.displayType}`);
      if (f.description) console.log(`    ${chalk.dim(f.description)}`);
    }
  }
  console.log();
}

/** Render rule instance list. */
export function renderRuleInstanceList(instances: ModelSecurityRuleInstance[]): void {
  if (instances.length === 0) {
    ui.emptyList('rule instances');
    return;
  }
  ui.section('Rule Instances:');
  for (const ri of instances) {
    const ruleName = (ri.rule as { name?: string })?.name ?? ri.securityRuleUuid;
    ui.dim(ri.uuid);
    console.log(`    ${ruleName}  ${stateColor(ri.state)(ri.state)}`);
  }
  console.log();
}

/** Render rule instance detail. */
export function renderRuleInstanceDetail(instance: ModelSecurityRuleInstance): void {
  ui.section('Rule Instance Detail:');
  const pairs: Array<[string, unknown]> = [
    ['UUID', instance.uuid],
    ['Group UUID', instance.securityGroupUuid],
    ['Rule UUID', instance.securityRuleUuid],
    ['State', stateColor(instance.state)(instance.state)],
  ];
  const ruleName = (instance.rule as { name?: string })?.name;
  if (ruleName) pairs.push(['Rule Name', ruleName]);
  pairs.push(['Created', instance.createdAt]);
  pairs.push(['Updated', instance.updatedAt]);
  ui.keyValue(pairs);

  if (Object.keys(instance.fieldValues).length > 0) {
    ui.section('Field Values:');
    ui.keyValue(
      Object.entries(instance.fieldValues).map(([key, value]) => [
        key,
        Array.isArray(value) ? value.join(', ') : String(value),
      ]),
    );
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
    ui.emptyList('scans');
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
  ui.section('Model Security Scans:');
  for (const s of scans) {
    ui.dim(s.uuid);
    console.log(
      `    ${stateColor(s.evalOutcome)(s.evalOutcome)}  ${chalk.dim(s.scanOrigin)}  ${chalk.dim(s.createdAt)}`,
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
  ui.section('Scan Detail:');
  const pairs: Array<[string, unknown]> = [
    ['UUID', scan.uuid],
    ['Outcome', stateColor(scan.evalOutcome)(scan.evalOutcome)],
  ];
  if (scan.modelUri) pairs.push(['Model URI', scan.modelUri]);
  pairs.push(['Origin', scan.scanOrigin]);
  pairs.push(['Source', scan.sourceType]);
  pairs.push(['Group', scan.securityGroupName]);
  pairs.push(['Created', scan.createdAt]);
  pairs.push(['Updated', scan.updatedAt]);
  if (scan.evalSummary) {
    const { rulesPassed, rulesFailed, totalRules } = scan.evalSummary;
    pairs.push([
      'Rules',
      `${chalk.green(`${rulesPassed} passed`)}  ${chalk.red(`${rulesFailed} failed`)}  / ${totalRules} total`,
    ]);
  }
  ui.keyValue(pairs);
  if (scan.labels.length > 0) {
    ui.section('Labels:');
    ui.keyValue(scan.labels.map((l) => [l.key, l.value]));
  }
  console.log();
}

// ---------------------------------------------------------------------------
// Model Security — Evaluations
// ---------------------------------------------------------------------------

/** Render a list of evaluations. */
export function renderEvaluationList(evaluations: ModelSecurityEvaluation[]): void {
  if (evaluations.length === 0) {
    ui.emptyList('evaluations');
    return;
  }
  ui.section('Rule Evaluations:');
  for (const e of evaluations) {
    ui.dim(e.uuid);
    console.log(
      `    ${e.ruleName}  ${stateColor(e.result)(e.result)}  ${chalk.dim(e.ruleInstanceState)}`,
    );
  }
  console.log();
}

/** Render a single evaluation detail. */
export function renderEvaluationDetail(evaluation: ModelSecurityEvaluation): void {
  ui.section('Evaluation Detail:');
  ui.keyValue([
    ['UUID', evaluation.uuid],
    ['Rule', evaluation.ruleName],
    ['Description', evaluation.ruleDescription],
    ['Instance UUID', evaluation.ruleInstanceUuid],
    ['Instance State', evaluation.ruleInstanceState],
    ['Result', stateColor(evaluation.result)(evaluation.result)],
    ['Violations', evaluation.violationCount],
  ]);
  console.log();
}

// ---------------------------------------------------------------------------
// Model Security — Violations
// ---------------------------------------------------------------------------

/** Render a list of violations. */
export function renderViolationList(violations: ModelSecurityViolation[]): void {
  if (violations.length === 0) {
    ui.emptyList('violations');
    return;
  }
  ui.section('Violations:');
  for (const v of violations) {
    ui.dim(v.uuid);
    console.log(`    ${chalk.red(v.ruleName)}  ${chalk.dim(v.file)}`);
    console.log(`    ${v.description}`);
    console.log(`    Threat: ${chalk.dim(v.threat)}`);
  }
  console.log();
}

/** Render a single violation detail. */
export function renderViolationDetail(violation: ModelSecurityViolation): void {
  ui.section('Violation Detail:');
  ui.keyValue([
    ['UUID', violation.uuid],
    ['Rule', chalk.red(violation.ruleName)],
    ['Description', violation.ruleDescription],
    ['State', violation.ruleInstanceState],
    ['File', violation.file],
    ['Threat', violation.threat],
    ['Detail', violation.description],
  ]);
  console.log();
}

// ---------------------------------------------------------------------------
// Model Security — Files
// ---------------------------------------------------------------------------

/** Render a list of scanned files. */
export function renderFileList(files: ModelSecurityFile[]): void {
  if (files.length === 0) {
    ui.emptyList('files');
    return;
  }
  ui.section('Scanned Files:');
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
    ui.emptyList('label keys');
    return;
  }
  ui.section('Label Keys:');
  for (const k of keys) {
    console.log(`  ${k}`);
  }
  console.log();
}

/** Render label values for a key. */
export function renderLabelValues(key: string, values: string[]): void {
  if (values.length === 0) {
    ui.emptyList(`values for key "${key}"`);
    return;
  }
  ui.section(`Label Values for "${key}":`);
  for (const v of values) {
    console.log(`  ${v}`);
  }
  console.log();
}
