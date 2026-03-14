import { execFileSync, spawn } from 'node:child_process';
import * as fs from 'node:fs';
import chalk from 'chalk';
import type { Command } from 'commander';
import { SdkModelSecurityService } from '../../airs/modelsecurity.js';
import { loadConfig } from '../../config/loader.js';
import {
  type OutputFormat,
  renderError,
  renderEvaluationDetail,
  renderEvaluationList,
  renderFileList,
  renderGroupDetail,
  renderGroupList,
  renderLabelKeys,
  renderLabelValues,
  renderModelSecurityHeader,
  renderMsScanDetail,
  renderMsScanList,
  renderRuleDetail,
  renderRuleInstanceDetail,
  renderRuleInstanceList,
  renderRuleList,
  renderViolationDetail,
  renderViolationList,
} from '../renderer/index.js';

const VALID_EXTRAS = ['all', 'aws', 'gcp', 'azure', 'artifactory', 'gitlab'] as const;

/** Check if a binary is available on PATH. */
function hasBin(bin: string): boolean {
  try {
    execFileSync(bin, ['--version'], { stdio: 'ignore' });
    return true;
  } catch {
    return false;
  }
}

/** Run a command with inherited stdio, rejecting on non-zero exit. */
function run(bin: string, args: string[], label: string): Promise<void> {
  return new Promise((resolve, reject) => {
    const child = spawn(bin, args, { stdio: 'inherit' });
    child.on('error', (err) => reject(new Error(`Failed to start ${bin}: ${err.message}`)));
    child.on('close', (code) => {
      if (code !== 0) reject(new Error(`${label} failed with exit code ${code}`));
      else resolve();
    });
  });
}

/** Create an SdkModelSecurityService from config. */
async function createService() {
  const config = await loadConfig();
  return new SdkModelSecurityService({
    clientId: config.mgmtClientId,
    clientSecret: config.mgmtClientSecret,
    tsgId: config.mgmtTsgId,
    tokenEndpoint: config.mgmtTokenEndpoint,
  });
}

/** Register the `model-security` command group. */
export function registerModelSecurityCommand(program: Command): void {
  const ms = program
    .command('model-security')
    .description('AI Model Security operations — groups, rules, scans');

  // -----------------------------------------------------------------------
  // model-security groups — security group CRUD
  // -----------------------------------------------------------------------
  const groups = ms.command('groups').description('Manage security groups');

  groups
    .command('list')
    .description('List security groups')
    .option('--source-types <types>', 'Filter by source types (comma-separated)')
    .option('--search <query>', 'Search by name or UUID')
    .option('--sort-field <field>', 'Sort field (created_at, updated_at)')
    .option('--sort-dir <dir>', 'Sort direction (asc, desc)')
    .option('--enabled-rules <uuids>', 'Filter by enabled rule UUIDs (comma-separated)')
    .option('--limit <n>', 'Max results', '20')
    .option('--output <format>', 'Output format: pretty, table, csv, json, yaml', 'pretty')
    .action(async (opts) => {
      try {
        const fmt = opts.output as OutputFormat;
        if (fmt === 'pretty') renderModelSecurityHeader();
        const service = await createService();
        const result = await service.listGroups({
          sourceTypes: opts.sourceTypes
            ? (opts.sourceTypes as string).split(',').map((s: string) => s.trim())
            : undefined,
          searchQuery: opts.search,
          sortField: opts.sortField,
          sortDir: opts.sortDir,
          enabledRules: opts.enabledRules
            ? (opts.enabledRules as string).split(',').map((s: string) => s.trim())
            : undefined,
          limit: Number.parseInt(opts.limit, 10),
        });
        renderGroupList(result.groups, fmt);
      } catch (err) {
        renderError(err instanceof Error ? err.message : String(err));
        process.exit(1);
      }
    });

  groups
    .command('get <uuid>')
    .description('Get security group details')
    .action(async (uuid: string) => {
      try {
        renderModelSecurityHeader();
        const service = await createService();
        const group = await service.getGroup(uuid);
        renderGroupDetail(group);
      } catch (err) {
        renderError(err instanceof Error ? err.message : String(err));
        process.exit(1);
      }
    });

  groups
    .command('create')
    .description('Create a security group')
    .requiredOption('--config <path>', 'JSON file with group configuration')
    .action(async (opts) => {
      try {
        renderModelSecurityHeader();
        const service = await createService();
        const config = JSON.parse(fs.readFileSync(opts.config, 'utf-8'));
        const group = await service.createGroup({
          name: config.name,
          sourceType: config.source_type,
          description: config.description,
          ruleConfigurations: config.rule_configurations,
        });
        console.log(`  Group created: ${group.uuid}\n`);
        renderGroupDetail(group);
      } catch (err) {
        renderError(err instanceof Error ? err.message : String(err));
        process.exit(1);
      }
    });

  groups
    .command('update <uuid>')
    .description('Update a security group')
    .option('--name <name>', 'New name')
    .option('--description <desc>', 'New description')
    .action(async (uuid: string, opts) => {
      try {
        renderModelSecurityHeader();
        const service = await createService();
        const request: { name?: string; description?: string } = {};
        if (opts.name) request.name = opts.name;
        if (opts.description) request.description = opts.description;
        const group = await service.updateGroup(uuid, request);
        console.log(`  Group updated: ${group.uuid}\n`);
        renderGroupDetail(group);
      } catch (err) {
        renderError(err instanceof Error ? err.message : String(err));
        process.exit(1);
      }
    });

  groups
    .command('delete <uuid>')
    .description('Delete a security group')
    .action(async (uuid: string) => {
      try {
        renderModelSecurityHeader();
        const service = await createService();
        await service.deleteGroup(uuid);
        console.log(`  Group ${uuid} deleted.\n`);
      } catch (err) {
        renderError(err instanceof Error ? err.message : String(err));
        process.exit(1);
      }
    });

  // -----------------------------------------------------------------------
  // model-security install — install model-security-client Python package
  // -----------------------------------------------------------------------
  ms.command('install')
    .description('Install the model-security-client Python package from AIRS PyPI')
    .option(
      '--extras <type>',
      'Source type extras to install (all, aws, gcp, azure, artifactory, gitlab)',
      'all',
    )
    .option('--dir <path>', 'Directory to create the project in', 'model-security')
    .option('--dry-run', 'Print the commands without executing')
    .action(async (opts) => {
      try {
        renderModelSecurityHeader();

        // Validate extras
        const extras = opts.extras as string;
        if (!VALID_EXTRAS.includes(extras as (typeof VALID_EXTRAS)[number])) {
          renderError(`Invalid extras "${extras}". Valid: ${VALID_EXTRAS.join(', ')}`);
          process.exit(1);
        }

        const dir = opts.dir as string;
        const useUv = hasBin('uv');

        if (!useUv && !hasBin('python3')) {
          renderError('Neither uv nor python3 found on PATH. Install one first.');
          process.exit(1);
        }

        // Fetch PyPI auth URL
        const service = await createService();
        const auth = await service.getPyPIAuth();
        const pkg = `model-security-client[${extras}]`;

        // Build step sequence
        type Step = { label: string; bin: string; args: string[] };
        const steps: Step[] = useUv
          ? [
              { label: 'uv init', bin: 'uv', args: ['init', dir] },
              {
                label: 'uv add',
                bin: 'uv',
                args: ['add', '--project', dir, pkg, '--index', auth.url],
              },
            ]
          : [
              { label: 'create venv', bin: 'python3', args: ['-m', 'venv', `${dir}/.venv`] },
              {
                label: 'pip install',
                bin: `${dir}/.venv/bin/pip`,
                args: ['install', pkg, '--extra-index-url', auth.url],
              },
            ];

        if (opts.dryRun) {
          console.log(chalk.bold('\n  Commands that would be executed:\n'));
          for (const step of steps) {
            const cmdStr = [step.bin, ...step.args]
              .map((a) => (a.includes('[') || a.includes(' ') ? `"${a}"` : a))
              .join(' ');
            console.log(`    ${chalk.dim('$')} ${cmdStr}`);
          }
          console.log();
          return;
        }

        for (const step of steps) {
          console.log(chalk.dim(`\n  → ${step.label}\n`));
          await run(step.bin, step.args, step.label);
        }

        console.log(chalk.green('\n  model-security-client installed successfully.\n'));

        if (useUv) {
          console.log(chalk.dim(`  Activate:  cd ${dir}\n`));
        } else {
          console.log(chalk.dim(`  Activate:  source ${dir}/.venv/bin/activate\n`));
        }
      } catch (err) {
        renderError(err instanceof Error ? err.message : String(err));
        process.exit(1);
      }
    });

  // -----------------------------------------------------------------------
  // model-security labels — scan label management
  // -----------------------------------------------------------------------
  const labels = ms.command('labels').description('Manage scan labels');

  labels
    .command('add <scanUuid>')
    .description('Add labels to a scan')
    .requiredOption('--labels <json>', 'JSON array of {key, value} labels')
    .action(async (scanUuid: string, opts) => {
      try {
        renderModelSecurityHeader();
        const service = await createService();
        const parsed = JSON.parse(opts.labels);
        await service.addLabels(scanUuid, parsed);
        console.log('  Labels added.\n');
      } catch (err) {
        renderError(err instanceof Error ? err.message : String(err));
        process.exit(1);
      }
    });

  labels
    .command('set <scanUuid>')
    .description('Replace all labels on a scan')
    .requiredOption('--labels <json>', 'JSON array of {key, value} labels')
    .action(async (scanUuid: string, opts) => {
      try {
        renderModelSecurityHeader();
        const service = await createService();
        const parsed = JSON.parse(opts.labels);
        await service.setLabels(scanUuid, parsed);
        console.log('  Labels set.\n');
      } catch (err) {
        renderError(err instanceof Error ? err.message : String(err));
        process.exit(1);
      }
    });

  labels
    .command('delete <scanUuid>')
    .description('Delete labels from a scan by key')
    .requiredOption('--keys <keys>', 'Comma-separated label keys to delete')
    .action(async (scanUuid: string, opts) => {
      try {
        renderModelSecurityHeader();
        const service = await createService();
        const keys = (opts.keys as string).split(',').map((k: string) => k.trim());
        await service.deleteLabels(scanUuid, keys);
        console.log('  Labels deleted.\n');
      } catch (err) {
        renderError(err instanceof Error ? err.message : String(err));
        process.exit(1);
      }
    });

  labels
    .command('keys')
    .description('List available label keys')
    .option('--limit <n>', 'Max results', '20')
    .action(async (opts) => {
      try {
        renderModelSecurityHeader();
        const service = await createService();
        const result = await service.getLabelKeys({
          limit: Number.parseInt(opts.limit, 10),
        });
        renderLabelKeys(result.keys);
      } catch (err) {
        renderError(err instanceof Error ? err.message : String(err));
        process.exit(1);
      }
    });

  labels
    .command('values <key>')
    .description('List values for a label key')
    .option('--limit <n>', 'Max results', '20')
    .action(async (key: string, opts) => {
      try {
        renderModelSecurityHeader();
        const service = await createService();
        const result = await service.getLabelValues(key, {
          limit: Number.parseInt(opts.limit, 10),
        });
        renderLabelValues(key, result.values);
      } catch (err) {
        renderError(err instanceof Error ? err.message : String(err));
        process.exit(1);
      }
    });

  // -----------------------------------------------------------------------
  // model-security pypi-auth — PyPI authentication for Google Artifact Registry
  // -----------------------------------------------------------------------
  ms.command('pypi-auth')
    .description('Get PyPI authentication URL for Google Artifact Registry')
    .action(async () => {
      try {
        renderModelSecurityHeader();
        const service = await createService();
        const auth = await service.getPyPIAuth();
        console.log(chalk.bold('\n  PyPI Authentication:\n'));
        console.log(`    URL:     ${auth.url}`);
        console.log(`    Expires: ${chalk.dim(auth.expiresAt)}`);
        console.log();
      } catch (err) {
        renderError(err instanceof Error ? err.message : String(err));
        process.exit(1);
      }
    });

  // -----------------------------------------------------------------------
  // model-security rule-instances — manage rule instances within groups
  // -----------------------------------------------------------------------
  const ruleInstances = ms.command('rule-instances').description('Manage rule instances in groups');

  ruleInstances
    .command('list <groupUuid>')
    .description('List rule instances in a security group')
    .option('--security-rule-uuid <uuid>', 'Filter by security rule UUID')
    .option('--state <state>', 'Filter by state (DISABLED, ALLOWING, BLOCKING)')
    .option('--limit <n>', 'Max results', '20')
    .action(async (groupUuid: string, opts) => {
      try {
        renderModelSecurityHeader();
        const service = await createService();
        const result = await service.listRuleInstances(groupUuid, {
          securityRuleUuid: opts.securityRuleUuid,
          state: opts.state,
          limit: Number.parseInt(opts.limit, 10),
        });
        renderRuleInstanceList(result.ruleInstances);
      } catch (err) {
        renderError(err instanceof Error ? err.message : String(err));
        process.exit(1);
      }
    });

  ruleInstances
    .command('get <groupUuid> <instanceUuid>')
    .description('Get rule instance details')
    .action(async (groupUuid: string, instanceUuid: string) => {
      try {
        renderModelSecurityHeader();
        const service = await createService();
        const instance = await service.getRuleInstance(groupUuid, instanceUuid);
        renderRuleInstanceDetail(instance);
      } catch (err) {
        renderError(err instanceof Error ? err.message : String(err));
        process.exit(1);
      }
    });

  ruleInstances
    .command('update <groupUuid> <instanceUuid>')
    .description('Update a rule instance')
    .requiredOption('--config <path>', 'JSON file with rule instance updates')
    .action(async (groupUuid: string, instanceUuid: string, opts) => {
      try {
        renderModelSecurityHeader();
        const service = await createService();
        const config = JSON.parse(fs.readFileSync(opts.config, 'utf-8'));
        const instance = await service.updateRuleInstance(groupUuid, instanceUuid, {
          state: config.state,
          fieldValues: config.field_values,
        });
        console.log(`  Rule instance updated: ${instance.uuid}\n`);
        renderRuleInstanceDetail(instance);
      } catch (err) {
        renderError(err instanceof Error ? err.message : String(err));
        process.exit(1);
      }
    });

  // -----------------------------------------------------------------------
  // model-security rules — browse security rules (read-only)
  // -----------------------------------------------------------------------
  const rules = ms.command('rules').description('Browse security rules');

  rules
    .command('list')
    .description('List available security rules')
    .option('--source-type <type>', 'Filter by source type')
    .option('--search <query>', 'Search by name or UUID')
    .option('--limit <n>', 'Max results', '20')
    .option('--output <format>', 'Output format: pretty, table, csv, json, yaml', 'pretty')
    .action(async (opts) => {
      try {
        const fmt = opts.output as OutputFormat;
        if (fmt === 'pretty') renderModelSecurityHeader();
        const service = await createService();
        const result = await service.listRules({
          sourceType: opts.sourceType,
          searchQuery: opts.search,
          limit: Number.parseInt(opts.limit, 10),
        });
        renderRuleList(result.rules, fmt);
      } catch (err) {
        renderError(err instanceof Error ? err.message : String(err));
        process.exit(1);
      }
    });

  rules
    .command('get <uuid>')
    .description('Get security rule details')
    .action(async (uuid: string) => {
      try {
        renderModelSecurityHeader();
        const service = await createService();
        const rule = await service.getRule(uuid);
        renderRuleDetail(rule);
      } catch (err) {
        renderError(err instanceof Error ? err.message : String(err));
        process.exit(1);
      }
    });

  // -----------------------------------------------------------------------
  // model-security scans — scan operations + results
  // -----------------------------------------------------------------------
  const scans = ms.command('scans').description('Model security scan operations');

  scans
    .command('list')
    .description('List model security scans')
    .option('--eval-outcome <outcome>', 'Filter by eval outcome')
    .option('--source-type <type>', 'Filter by source type')
    .option('--scan-origin <origin>', 'Filter by scan origin')
    .option('--search <query>', 'Search scans')
    .option('--limit <n>', 'Max results', '20')
    .option('--output <format>', 'Output format: pretty, table, csv, json, yaml', 'pretty')
    .action(async (opts) => {
      try {
        const fmt = opts.output as OutputFormat;
        if (fmt === 'pretty') renderModelSecurityHeader();
        const service = await createService();
        const result = await service.listScans({
          evalOutcome: opts.evalOutcome,
          sourceType: opts.sourceType,
          scanOrigin: opts.scanOrigin,
          search: opts.search,
          limit: Number.parseInt(opts.limit, 10),
        });
        renderMsScanList(result.scans, fmt);
      } catch (err) {
        renderError(err instanceof Error ? err.message : String(err));
        process.exit(1);
      }
    });

  scans
    .command('get <uuid>')
    .description('Get scan details')
    .action(async (uuid: string) => {
      try {
        renderModelSecurityHeader();
        const service = await createService();
        const scan = await service.getScan(uuid);
        renderMsScanDetail(scan);
      } catch (err) {
        renderError(err instanceof Error ? err.message : String(err));
        process.exit(1);
      }
    });

  scans
    .command('create')
    .description('Create a model security scan')
    .requiredOption('--config <path>', 'JSON file with scan configuration')
    .action(async (opts) => {
      try {
        renderModelSecurityHeader();
        const service = await createService();
        const config = JSON.parse(fs.readFileSync(opts.config, 'utf-8'));
        const scan = await service.createScan(config);
        console.log(`  Scan created: ${scan.uuid}\n`);
        renderMsScanDetail(scan);
      } catch (err) {
        renderError(err instanceof Error ? err.message : String(err));
        process.exit(1);
      }
    });

  scans
    .command('evaluations <scanUuid>')
    .description('List rule evaluations for a scan')
    .option('--limit <n>', 'Max results', '20')
    .action(async (scanUuid: string, opts) => {
      try {
        renderModelSecurityHeader();
        const service = await createService();
        const result = await service.getEvaluations(scanUuid, {
          limit: Number.parseInt(opts.limit, 10),
        });
        renderEvaluationList(result.evaluations);
      } catch (err) {
        renderError(err instanceof Error ? err.message : String(err));
        process.exit(1);
      }
    });

  scans
    .command('evaluation <uuid>')
    .description('Get evaluation details')
    .action(async (uuid: string) => {
      try {
        renderModelSecurityHeader();
        const service = await createService();
        const evaluation = await service.getEvaluation(uuid);
        renderEvaluationDetail(evaluation);
      } catch (err) {
        renderError(err instanceof Error ? err.message : String(err));
        process.exit(1);
      }
    });

  scans
    .command('violations <scanUuid>')
    .description('List violations for a scan')
    .option('--limit <n>', 'Max results', '20')
    .action(async (scanUuid: string, opts) => {
      try {
        renderModelSecurityHeader();
        const service = await createService();
        const result = await service.getViolations(scanUuid, {
          limit: Number.parseInt(opts.limit, 10),
        });
        renderViolationList(result.violations);
      } catch (err) {
        renderError(err instanceof Error ? err.message : String(err));
        process.exit(1);
      }
    });

  scans
    .command('violation <uuid>')
    .description('Get violation details')
    .action(async (uuid: string) => {
      try {
        renderModelSecurityHeader();
        const service = await createService();
        const violation = await service.getViolation(uuid);
        renderViolationDetail(violation);
      } catch (err) {
        renderError(err instanceof Error ? err.message : String(err));
        process.exit(1);
      }
    });

  scans
    .command('files <scanUuid>')
    .description('List scanned files')
    .option('--type <type>', 'Filter by file type')
    .option('--result <result>', 'Filter by result')
    .option('--limit <n>', 'Max results', '20')
    .action(async (scanUuid: string, opts) => {
      try {
        renderModelSecurityHeader();
        const service = await createService();
        const result = await service.getFiles(scanUuid, {
          type: opts.type,
          result: opts.result,
          limit: Number.parseInt(opts.limit, 10),
        });
        renderFileList(result.files);
      } catch (err) {
        renderError(err instanceof Error ? err.message : String(err));
        process.exit(1);
      }
    });
}
