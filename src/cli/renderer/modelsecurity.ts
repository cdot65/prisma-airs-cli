import chalk from 'chalk';
import type {
  ModelSecurityEvaluation,
  ModelSecurityFile,
  ModelSecurityGroup,
  ModelSecurityModel,
  ModelSecurityModelVersion,
  ModelSecurityRule,
  ModelSecurityRuleInstance,
  ModelSecurityScan,
  ModelSecurityViolation,
} from '../../airs/types.js';
import type { OutputFormat } from './common.js';
import { ui } from './ui.js';
import { emitDetail, emitList, type ResourceView } from './view.js';

function resourceView<T>(
  name: string,
  columns: ResourceView<T>['columns'],
  pretty: ResourceView<T>['pretty'],
): ResourceView<T> {
  return { name, columns, pretty };
}

function structuredList<T>(
  name: string,
  items: T[],
  columns: ResourceView<T>['columns'],
  format: OutputFormat,
  pretty: () => void,
): void {
  emitList(resourceView(name, columns, { list: pretty, detail: () => undefined }), items, format);
}

function structuredDetail<T>(
  name: string,
  item: T,
  format: OutputFormat,
  pretty: () => void,
): void {
  emitDetail(resourceView(name, [], { list: () => undefined, detail: pretty }), item, format);
}

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
  if (format !== 'pretty') {
    structuredList(
      'security groups',
      groups,
      [
        { key: 'uuid', label: 'ID' },
        { key: 'name', label: 'Name' },
        { key: 'state', label: 'State' },
        { key: 'sourceType', label: 'Source Type' },
      ],
      format,
      () => undefined,
    );
    return;
  }
  if (groups.length === 0) {
    ui.emptyList('security groups');
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
  if (format !== 'pretty') {
    structuredDetail('security group', group, format, () => undefined);
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
  if (format !== 'pretty') {
    structuredList(
      'security rules',
      rules,
      [
        { key: 'uuid', label: 'ID' },
        { key: 'name', label: 'Name' },
        { key: 'ruleType', label: 'Type' },
        { key: 'defaultState', label: 'Default State' },
        {
          key: 'compatibleSources',
          label: 'Sources',
          get: (rule) => rule.compatibleSources.join(', '),
        },
      ],
      format,
      () => undefined,
    );
    return;
  }
  if (rules.length === 0) {
    ui.emptyList('security rules');
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
export function renderRuleDetail(rule: ModelSecurityRule, format: OutputFormat = 'pretty'): void {
  if (format !== 'pretty') {
    structuredDetail('security rule', rule, format, () => undefined);
    return;
  }
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
export function renderRuleInstanceList(
  instances: ModelSecurityRuleInstance[],
  format: OutputFormat = 'pretty',
): void {
  if (format !== 'pretty') {
    structuredList(
      'rule instances',
      instances,
      [
        { key: 'uuid', label: 'ID' },
        { key: 'securityRuleUuid', label: 'Rule ID' },
        { key: 'state', label: 'State' },
      ],
      format,
      () => undefined,
    );
    return;
  }
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
export function renderRuleInstanceDetail(
  instance: ModelSecurityRuleInstance,
  format: OutputFormat = 'pretty',
): void {
  if (format !== 'pretty') {
    structuredDetail('rule instance', instance, format, () => undefined);
    return;
  }
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
  if (format !== 'pretty') {
    structuredList(
      'scans',
      scans,
      [
        { key: 'uuid', label: 'ID' },
        { key: 'evalOutcome', label: 'Outcome' },
        { key: 'scanOrigin', label: 'Origin' },
        { key: 'modelUri', label: 'Model URI' },
        { key: 'createdAt', label: 'Created' },
      ],
      format,
      () => undefined,
    );
    return;
  }
  if (scans.length === 0) {
    ui.emptyList('scans');
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
export function renderMsScanDetail(scan: ModelSecurityScan, format: OutputFormat = 'pretty'): void {
  if (format !== 'pretty') {
    structuredDetail('scan', scan, format, () => undefined);
    return;
  }
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
export function renderEvaluationList(
  evaluations: ModelSecurityEvaluation[],
  format: OutputFormat = 'pretty',
): void {
  if (format !== 'pretty') {
    structuredList(
      'evaluations',
      evaluations,
      [
        { key: 'uuid', label: 'ID' },
        { key: 'ruleName', label: 'Rule' },
        { key: 'result', label: 'Result' },
        { key: 'ruleInstanceState', label: 'State' },
      ],
      format,
      () => undefined,
    );
    return;
  }
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
export function renderEvaluationDetail(
  evaluation: ModelSecurityEvaluation,
  format: OutputFormat = 'pretty',
): void {
  if (format !== 'pretty') {
    structuredDetail('evaluation', evaluation, format, () => undefined);
    return;
  }
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
export function renderViolationList(
  violations: ModelSecurityViolation[],
  format: OutputFormat = 'pretty',
): void {
  if (format !== 'pretty') {
    structuredList(
      'violations',
      violations,
      [
        { key: 'uuid', label: 'ID' },
        { key: 'ruleName', label: 'Rule' },
        { key: 'file', label: 'File' },
        { key: 'threat', label: 'Threat' },
      ],
      format,
      () => undefined,
    );
    return;
  }
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
export function renderViolationDetail(
  violation: ModelSecurityViolation,
  format: OutputFormat = 'pretty',
): void {
  if (format !== 'pretty') {
    structuredDetail('violation', violation, format, () => undefined);
    return;
  }
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
export function renderFileList(files: ModelSecurityFile[], format: OutputFormat = 'pretty'): void {
  if (format !== 'pretty') {
    structuredList(
      'files',
      files,
      [
        { key: 'uuid', label: 'ID' },
        { key: 'path', label: 'Path' },
        { key: 'type', label: 'Type' },
        { key: 'result', label: 'Result' },
      ],
      format,
      () => undefined,
    );
    return;
  }
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

// ---------------------------------------------------------------------------
// Model Security — Model catalog (read-only)
// ---------------------------------------------------------------------------

/** Render a list of catalog models. */
export function renderModelList(
  models: ModelSecurityModel[],
  format: OutputFormat = 'pretty',
): void {
  if (format !== 'pretty') {
    structuredList(
      'models',
      models,
      [
        { key: 'uuid', label: 'ID' },
        { key: 'name', label: 'Name' },
        { key: 'latestVersionOutcome', label: 'Outcome' },
        {
          key: 'latestVersionFormats',
          label: 'Formats',
          get: (model) => (model.latestVersionFormats ?? []).join(', '),
        },
        { key: 'latestVersionScanTime', label: 'Last Scan' },
      ],
      format,
      () => undefined,
    );
    return;
  }
  if (models.length === 0) {
    ui.emptyList('models');
    return;
  }
  ui.section('Models:');
  for (const m of models) {
    ui.dim(m.uuid);
    const outcome = m.latestVersionOutcome
      ? stateColor(m.latestVersionOutcome)(m.latestVersionOutcome)
      : chalk.dim('unscanned');
    const formats =
      m.latestVersionFormats && m.latestVersionFormats.length > 0
        ? chalk.dim(` [${m.latestVersionFormats.join(', ')}]`)
        : '';
    console.log(`    ${m.name}  ${outcome}${formats}`);
    console.log();
  }
}

/** Render a single model's detail. */
export function renderModelDetail(
  model: ModelSecurityModel,
  format: OutputFormat = 'pretty',
): void {
  if (format !== 'pretty') {
    structuredDetail('model', model, format, () => undefined);
    return;
  }
  ui.section('Model Detail:');
  const pairs: Array<[string, unknown]> = [
    ['UUID', model.uuid],
    ['Name', model.name],
    ['Created', model.createdAt],
    ['Updated', model.updatedAt],
  ];
  if (model.latestVersionUuid != null) pairs.push(['Latest Version', model.latestVersionUuid]);
  if (model.latestVersionRevision != null)
    pairs.push(['Latest Revision', model.latestVersionRevision]);
  if (model.latestVersionOutcome != null)
    pairs.push([
      'Latest Outcome',
      stateColor(model.latestVersionOutcome)(model.latestVersionOutcome),
    ]);
  if (model.latestVersionFormats?.length)
    pairs.push(['Formats', model.latestVersionFormats.join(', ')]);
  if (model.latestVersionSourceTypes?.length)
    pairs.push(['Source Types', model.latestVersionSourceTypes.join(', ')]);
  if (model.latestVersionScanTime != null) pairs.push(['Last Scan', model.latestVersionScanTime]);
  ui.keyValue(pairs);
  console.log();
}

/** Render a list of model versions. */
export function renderModelVersionList(
  versions: ModelSecurityModelVersion[],
  format: OutputFormat = 'pretty',
): void {
  if (format !== 'pretty') {
    structuredList(
      'versions',
      versions,
      [
        { key: 'uuid', label: 'ID' },
        { key: 'revision', label: 'Revision' },
        { key: 'fileCount', label: 'Files' },
        { key: 'lastEvalOutcome', label: 'Outcome' },
        { key: 'latestScanTime', label: 'Last Scan' },
      ],
      format,
      () => undefined,
    );
    return;
  }
  if (versions.length === 0) {
    ui.emptyList('versions');
    return;
  }
  ui.section('Model Versions:');
  for (const v of versions) {
    ui.dim(v.uuid);
    const outcome = v.lastEvalOutcome
      ? stateColor(v.lastEvalOutcome)(v.lastEvalOutcome)
      : chalk.dim('unscanned');
    const files = v.fileCount != null ? `  files: ${v.fileCount}` : '';
    console.log(`    ${v.revision}  ${outcome}${files}`);
    console.log();
  }
}

/** Render a single model version's detail. */
export function renderModelVersionDetail(
  version: ModelSecurityModelVersion,
  format: OutputFormat = 'pretty',
): void {
  if (format !== 'pretty') {
    structuredDetail('model version', version, format, () => undefined);
    return;
  }
  ui.section('Model Version Detail:');
  const pairs: Array<[string, unknown]> = [
    ['UUID', version.uuid],
    ['Model', version.modelUuid],
    ['Revision', version.revision],
    ['Created', version.createdAt],
    ['Updated', version.updatedAt],
  ];
  if (version.fileCount != null) pairs.push(['File Count', version.fileCount]);
  if (version.license != null) pairs.push(['License', version.license]);
  if (version.modelFormats?.length) pairs.push(['Formats', version.modelFormats.join(', ')]);
  if (version.sourceTypes?.length) pairs.push(['Source Types', version.sourceTypes.join(', ')]);
  if (version.hfModelName != null) pairs.push(['HF Model', version.hfModelName]);
  if (version.hfOrganization != null) pairs.push(['HF Organization', version.hfOrganization]);
  if (version.lastEvalOutcome != null)
    pairs.push(['Last Outcome', stateColor(version.lastEvalOutcome)(version.lastEvalOutcome)]);
  if (version.latestScanTime != null) pairs.push(['Last Scan', version.latestScanTime]);
  ui.keyValue(pairs);
  if (version.lastEvalSummary) {
    ui.section('Last Eval Summary:');
    ui.keyValue([
      ['Passed', version.lastEvalSummary.rulesPassed],
      ['Failed', version.lastEvalSummary.rulesFailed],
      ['Total', version.lastEvalSummary.totalRules],
    ]);
  }
  console.log();
}

/** Render a list of model-version files (format-aware). */
export function renderModelFileList(
  files: ModelSecurityFile[],
  format: OutputFormat = 'pretty',
): void {
  if (format !== 'pretty') {
    renderFileList(files, format);
    return;
  }
  if (files.length === 0) {
    ui.emptyList('files');
    return;
  }
  renderFileList(files);
}
