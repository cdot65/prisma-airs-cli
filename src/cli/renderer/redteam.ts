import chalk from 'chalk';

/** Render the red team banner. */
export function renderRedteamHeader(): void {
  console.log(chalk.bold.red('\n  Prisma AIRS — AI Red Team'));
  console.log(chalk.dim('  Adversarial scan operations\n'));
}

type ChalkFn = (text: string) => string;

/** Severity → chalk color mapping. */
function severityColor(severity: string): ChalkFn {
  switch (severity.toUpperCase()) {
    case 'CRITICAL':
      return chalk.red;
    case 'HIGH':
      return chalk.magenta;
    case 'MEDIUM':
      return chalk.yellow;
    case 'LOW':
      return chalk.cyan;
    default:
      return chalk.dim;
  }
}

/** Status → chalk color mapping. */
function statusColor(status: string): ChalkFn {
  switch (status) {
    case 'COMPLETED':
      return chalk.green;
    case 'RUNNING':
      return chalk.blue;
    case 'QUEUED':
    case 'INIT':
      return chalk.yellow;
    case 'FAILED':
    case 'ABORTED':
      return chalk.red;
    case 'PARTIALLY_COMPLETE':
      return chalk.yellow;
    default:
      return chalk.white;
  }
}

/** Render a scan's status summary. */
export function renderScanStatus(job: {
  uuid: string;
  name: string;
  status: string;
  jobType: string;
  targetName?: string;
  score?: number | null;
  asr?: number | null;
  completed?: number | null;
  total?: number | null;
}): void {
  console.log(chalk.bold('  Scan Status:'));
  console.log(`    ID:      ${chalk.dim(job.uuid)}`);
  console.log(`    Name:    ${job.name}`);
  console.log(`    Type:    ${job.jobType}`);
  if (job.targetName) console.log(`    Target:  ${job.targetName}`);
  console.log(`    Status:  ${statusColor(job.status)(job.status)}`);
  if (job.total != null && job.completed != null) {
    console.log(`    Progress: ${job.completed}/${job.total}`);
  }
  if (job.score != null) console.log(`    Score:   ${job.score}`);
  if (job.asr != null) console.log(`    ASR:     ${job.asr.toFixed(1)}%`);
  console.log();
}

/** Render a table of scans. */
export function renderScanList(
  jobs: Array<{
    uuid: string;
    name: string;
    status: string;
    jobType: string;
    score?: number | null;
    createdAt?: string | null;
  }>,
): void {
  if (jobs.length === 0) {
    console.log(chalk.dim('  No scans found.\n'));
    return;
  }
  console.log(chalk.bold('\n  Recent Scans:\n'));
  for (const job of jobs) {
    console.log(`  ${chalk.dim(job.uuid)}`);
    console.log(
      `    ${job.name}  ${statusColor(job.status)(job.status)}  ${job.jobType}${job.score != null ? `  score: ${job.score}` : ''}`,
    );
    if (job.createdAt) console.log(`    ${chalk.dim(job.createdAt)}`);
    console.log();
  }
}

/** Render a static scan report. */
export function renderStaticReport(report: {
  score?: number | null;
  asr?: number | null;
  severityBreakdown: Array<{ severity: string; successful: number; failed: number }>;
  reportSummary?: string | null;
  categories: Array<{
    id: string;
    displayName: string;
    asr: number;
    successful: number;
    failed: number;
    total: number;
  }>;
}): void {
  console.log(chalk.bold('\n  Static Scan Report:'));
  if (report.score != null) console.log(`    Score: ${report.score}`);
  if (report.asr != null) console.log(`    ASR:   ${report.asr.toFixed(1)}%`);

  if (report.severityBreakdown.length > 0) {
    console.log(chalk.bold('\n  Severity Breakdown:'));
    for (const s of report.severityBreakdown) {
      const color = severityColor(s.severity);
      console.log(
        `    ${color(s.severity.padEnd(10))} ${chalk.red(`${s.successful} bypassed`)}  ${chalk.green(`${s.failed} blocked`)}`,
      );
    }
  }

  if (report.categories.length > 0) {
    console.log(chalk.bold('\n  Categories:'));
    for (const c of report.categories) {
      console.log(
        `    ${c.displayName.padEnd(30)} ASR: ${c.asr.toFixed(1)}%  (${c.successful}/${c.total})`,
      );
    }
  }

  if (report.reportSummary) {
    console.log(chalk.bold('\n  Summary:'));
    console.log(`    ${report.reportSummary}`);
  }
  console.log();
}

/** Render a custom attack report. */
export function renderCustomReport(report: {
  totalPrompts: number;
  totalAttacks: number;
  totalThreats: number;
  score: number;
  asr: number;
  promptSets: Array<{
    promptSetName: string;
    totalPrompts: number;
    totalThreats: number;
    threatRate: number;
  }>;
}): void {
  console.log(chalk.bold('\n  Custom Attack Report:'));
  console.log(`    Score:   ${report.score}`);
  console.log(`    ASR:     ${report.asr.toFixed(1)}%`);
  console.log(`    Attacks: ${report.totalAttacks}  Threats: ${report.totalThreats}`);

  if (report.promptSets.length > 0) {
    console.log(chalk.bold('\n  Prompt Sets:'));
    for (const ps of report.promptSets) {
      console.log(
        `    ${ps.promptSetName.padEnd(40)} ${ps.totalThreats}/${ps.totalPrompts} threats  (${ps.threatRate.toFixed(1)}%)`,
      );
    }
  }
  console.log();
}

/** Render attack list with severity coloring. */
export function renderAttackList(
  attacks: Array<{
    name: string;
    severity?: string;
    category?: string;
    successful: boolean;
  }>,
): void {
  if (attacks.length === 0) {
    console.log(chalk.dim('  No attacks found.\n'));
    return;
  }
  console.log(chalk.bold('\n  Attacks:\n'));
  for (const a of attacks) {
    const sev = a.severity
      ? severityColor(a.severity)(a.severity.padEnd(10))
      : chalk.dim('N/A'.padEnd(10));
    const result = a.successful ? chalk.red('BYPASSED') : chalk.green('BLOCKED');
    console.log(
      `    ${sev} ${result}  ${a.name}${a.category ? chalk.dim(` [${a.category}]`) : ''}`,
    );
  }
  console.log();
}

/** Render custom attack list (prompt-level results). */
export function renderCustomAttackList(
  attacks: Array<{
    promptText: string;
    goal?: string;
    threat: boolean;
    asr?: number;
    promptSetName?: string;
  }>,
): void {
  if (attacks.length === 0) {
    console.log(chalk.dim('  No custom attacks found.\n'));
    return;
  }
  console.log(chalk.bold('\n  Custom Attacks:\n'));
  for (const a of attacks) {
    const result = a.threat ? chalk.red('THREAT') : chalk.green('SAFE');
    const prompt = a.promptText.length > 80 ? `${a.promptText.substring(0, 77)}...` : a.promptText;
    const asrStr = a.asr != null ? chalk.dim(` ASR: ${a.asr.toFixed(1)}%`) : '';
    console.log(`    ${result}${asrStr}  ${prompt}`);
    if (a.goal) console.log(`      ${chalk.dim(a.goal)}`);
  }
  console.log();
}

/** Render target list. */
export function renderTargetList(
  targets: Array<{
    uuid: string;
    name: string;
    status: string;
    targetType?: string;
    active: boolean;
  }>,
): void {
  if (targets.length === 0) {
    console.log(chalk.dim('  No targets found.\n'));
    return;
  }
  console.log(chalk.bold('\n  Targets:\n'));
  for (const t of targets) {
    console.log(`  ${chalk.dim(t.uuid)}`);
    console.log(
      `    ${t.name}  ${statusColor(t.active ? 'COMPLETED' : 'FAILED')(t.active ? 'active' : 'inactive')}${t.targetType ? `  type: ${t.targetType}` : ''}`,
    );
  }
  console.log();
}

/** Render attack category tree. */
export function renderCategories(
  categories: Array<{
    id: string;
    displayName: string;
    description?: string;
    subCategories: Array<{ id: string; displayName: string; description?: string }>;
  }>,
): void {
  if (categories.length === 0) {
    console.log(chalk.dim('  No categories found.\n'));
    return;
  }
  console.log(chalk.bold('\n  Attack Categories:\n'));
  for (const c of categories) {
    console.log(
      `  ${chalk.bold(c.displayName)}${c.description ? chalk.dim(` — ${c.description}`) : ''}`,
    );
    for (const sc of c.subCategories) {
      console.log(
        `    ${chalk.dim('•')} ${sc.displayName}${sc.description ? chalk.dim(` — ${sc.description}`) : ''}`,
      );
    }
    console.log();
  }
}

/** Render prompt set list. */
export function renderPromptSetList(
  promptSets: Array<{ uuid: string; name: string; active: boolean }>,
): void {
  if (promptSets.length === 0) {
    console.log(chalk.dim('  No prompt sets found.\n'));
    return;
  }
  console.log(chalk.bold('\n  Prompt Sets:\n'));
  for (const ps of promptSets) {
    console.log(`  ${chalk.dim(ps.uuid)}`);
    console.log(
      `    ${ps.name}  ${statusColor(ps.active ? 'COMPLETED' : 'FAILED')(ps.active ? 'active' : 'inactive')}`,
    );
  }
  console.log();
}

/** Render target detail. */
export function renderTargetDetail(target: {
  uuid: string;
  name: string;
  status: string;
  targetType?: string;
  active: boolean;
  connectionParams?: Record<string, unknown>;
  background?: Record<string, unknown>;
  metadata?: Record<string, unknown>;
}): void {
  console.log(chalk.bold('\n  Target Detail:\n'));
  console.log(`    UUID:   ${chalk.dim(target.uuid)}`);
  console.log(`    Name:   ${target.name}`);
  console.log(
    `    Status: ${statusColor(target.active ? 'COMPLETED' : 'FAILED')(target.active ? 'active' : 'inactive')}`,
  );
  if (target.targetType) console.log(`    Type:   ${target.targetType}`);
  if (target.connectionParams) {
    console.log(chalk.bold('\n    Connection:'));
    for (const [k, v] of Object.entries(target.connectionParams)) {
      console.log(`      ${k}: ${chalk.dim(String(v))}`);
    }
  }
  if (target.background) {
    console.log(chalk.bold('\n    Background:'));
    for (const [k, v] of Object.entries(target.background)) {
      if (v != null) console.log(`      ${k}: ${chalk.dim(String(v))}`);
    }
  }
  if (target.metadata) {
    console.log(chalk.bold('\n    Metadata:'));
    for (const [k, v] of Object.entries(target.metadata)) {
      if (v != null) console.log(`      ${k}: ${chalk.dim(String(v))}`);
    }
  }
  console.log();
}

/** Render prompt set detail. */
export function renderPromptSetDetail(ps: {
  uuid: string;
  name: string;
  active: boolean;
  archive: boolean;
  description?: string;
  createdAt?: string;
  updatedAt?: string;
}): void {
  console.log(chalk.bold('\n  Prompt Set Detail:\n'));
  console.log(`    UUID:        ${chalk.dim(ps.uuid)}`);
  console.log(`    Name:        ${ps.name}`);
  console.log(
    `    Status:      ${statusColor(ps.active ? 'COMPLETED' : 'FAILED')(ps.active ? 'active' : 'inactive')}`,
  );
  console.log(`    Archived:    ${ps.archive ? 'yes' : 'no'}`);
  if (ps.description) console.log(`    Description: ${ps.description}`);
  if (ps.createdAt) console.log(`    Created:     ${chalk.dim(ps.createdAt)}`);
  if (ps.updatedAt) console.log(`    Updated:     ${chalk.dim(ps.updatedAt)}`);
  console.log();
}

/** Render prompt set version info. */
export function renderVersionInfo(info: {
  uuid: string;
  version: number;
  stats: { total: number; active: number; inactive: number };
}): void {
  console.log(chalk.bold('\n  Version Info:\n'));
  console.log(`    Version:  ${info.version}`);
  console.log(`    Total:    ${info.stats.total}`);
  console.log(`    Active:   ${chalk.green(String(info.stats.active))}`);
  console.log(`    Inactive: ${chalk.dim(String(info.stats.inactive))}`);
  console.log();
}

/** Render a list of prompts. */
export function renderPromptList(
  prompts: Array<{
    uuid: string;
    prompt: string;
    goal?: string;
    active: boolean;
  }>,
): void {
  if (prompts.length === 0) {
    console.log(chalk.dim('  No prompts found.\n'));
    return;
  }
  console.log(chalk.bold('\n  Prompts:\n'));
  for (const p of prompts) {
    const status = p.active ? chalk.green('active') : chalk.dim('inactive');
    const text = p.prompt.length > 80 ? `${p.prompt.substring(0, 77)}...` : p.prompt;
    console.log(`  ${chalk.dim(p.uuid)}  ${status}`);
    console.log(`    ${text}`);
    if (p.goal) console.log(`    ${chalk.dim(`Goal: ${p.goal}`)}`);
  }
  console.log();
}

/** Render prompt detail. */
export function renderPromptDetail(p: {
  uuid: string;
  prompt: string;
  goal?: string;
  active: boolean;
  promptSetId: string;
}): void {
  console.log(chalk.bold('\n  Prompt Detail:\n'));
  console.log(`    UUID:       ${chalk.dim(p.uuid)}`);
  console.log(`    Set UUID:   ${chalk.dim(p.promptSetId)}`);
  console.log(`    Status:     ${p.active ? chalk.green('active') : chalk.dim('inactive')}`);
  console.log(`    Prompt:     ${p.prompt}`);
  if (p.goal) console.log(`    Goal:       ${p.goal}`);
  console.log();
}

/** Render property names list. */
export function renderPropertyNames(names: Array<{ name: string }>): void {
  if (names.length === 0) {
    console.log(chalk.dim('  No property names found.\n'));
    return;
  }
  console.log(chalk.bold('\n  Property Names:\n'));
  for (const n of names) {
    console.log(`    ${chalk.dim('•')} ${n.name}`);
  }
  console.log();
}

/** Render property values. */
export function renderPropertyValues(values: Array<{ name: string; value: string }>): void {
  if (values.length === 0) {
    console.log(chalk.dim('  No property values found.\n'));
    return;
  }
  console.log(chalk.bold('\n  Property Values:\n'));
  for (const v of values) {
    console.log(`    ${v.name}: ${chalk.dim(v.value)}`);
  }
  console.log();
}
