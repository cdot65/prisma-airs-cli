import * as fs from 'node:fs';
import type { Command } from 'commander';
import { SdkPromptSetService } from '../../airs/promptsets.js';
import { SdkRedTeamService } from '../../airs/redteam.js';
import { loadConfig } from '../../config/loader.js';
import {
  OUTPUT_FORMATS,
  type OutputFormat,
  renderAttackList,
  renderCategories,
  renderCustomAttackList,
  renderCustomReport,
  renderError,
  renderPromptDetail,
  renderPromptList,
  renderPromptSetDetail,
  renderPromptSetList,
  renderPropertyNames,
  renderPropertyValues,
  renderRedteamHeader,
  renderScanList,
  renderScanProgress,
  renderScanStatus,
  renderStaticReport,
  renderTargetDetail,
  renderTargetList,
  renderVersionInfo,
} from '../renderer/index.js';

/** Create an SdkRedTeamService from config. */
async function createService() {
  const config = await loadConfig();
  return new SdkRedTeamService({
    clientId: config.mgmtClientId,
    clientSecret: config.mgmtClientSecret,
    tsgId: config.mgmtTsgId,
    tokenEndpoint: config.mgmtTokenEndpoint,
  });
}

/** Create an SdkPromptSetService from config. */
async function createPromptSetService() {
  const config = await loadConfig();
  return new SdkPromptSetService({
    clientId: config.mgmtClientId,
    clientSecret: config.mgmtClientSecret,
    tsgId: config.mgmtTsgId,
    tokenEndpoint: config.mgmtTokenEndpoint,
  });
}

/** Register the `redteam` command group. */
export function registerRedteamCommand(program: Command): void {
  const redteam = program.command('redteam').description('AI Red Team scan operations');

  // -----------------------------------------------------------------------
  // redteam abort — abort a running scan
  // -----------------------------------------------------------------------
  redteam
    .command('abort <jobId>')
    .description('Abort a running scan')
    .action(async (jobId: string) => {
      try {
        renderRedteamHeader();
        const service = await createService();
        await service.abortScan(jobId);
        console.log(`  Scan ${jobId} aborted.\n`);
      } catch (err) {
        renderError(err instanceof Error ? err.message : String(err));
        process.exit(1);
      }
    });

  // -----------------------------------------------------------------------
  // redteam categories — list attack categories
  // -----------------------------------------------------------------------
  redteam
    .command('categories')
    .description('List available attack categories')
    .action(async () => {
      try {
        renderRedteamHeader();
        const service = await createService();
        const categories = await service.getCategories();
        renderCategories(categories);
      } catch (err) {
        renderError(err instanceof Error ? err.message : String(err));
        process.exit(1);
      }
    });

  // -----------------------------------------------------------------------
  // redteam list — list recent scans
  // -----------------------------------------------------------------------
  redteam
    .command('list')
    .description('List recent scans')
    .option('--status <status>', 'Filter by status (QUEUED, RUNNING, COMPLETED, FAILED, ABORTED)')
    .option('--type <type>', 'Filter by job type (STATIC, DYNAMIC, CUSTOM)')
    .option('--target <uuid>', 'Filter by target UUID')
    .option('--limit <n>', 'Max results', '10')
    .option('--output <format>', 'Output format: pretty, table, csv, json, yaml', 'pretty')
    .action(async (opts) => {
      try {
        const fmt = opts.output as OutputFormat;
        if (fmt === 'pretty') renderRedteamHeader();
        const service = await createService();
        const scans = await service.listScans({
          status: opts.status,
          jobType: opts.type,
          targetId: opts.target,
          limit: Number.parseInt(opts.limit, 10),
        });
        renderScanList(scans, fmt);
      } catch (err) {
        renderError(err instanceof Error ? err.message : String(err));
        process.exit(1);
      }
    });

  // -----------------------------------------------------------------------
  // redteam prompt-sets — prompt set CRUD subcommands
  // -----------------------------------------------------------------------
  const promptSets = redteam.command('prompt-sets').description('Manage custom prompt sets');

  promptSets
    .command('list')
    .description('List custom prompt sets')
    .option('--output <format>', 'Output format: pretty, table, csv, json, yaml', 'pretty')
    .action(async (opts) => {
      try {
        const fmt = opts.output as OutputFormat;
        if (fmt === 'pretty') renderRedteamHeader();
        const service = await createPromptSetService();
        const sets = await service.listPromptSets();
        renderPromptSetList(sets, fmt);
      } catch (err) {
        renderError(err instanceof Error ? err.message : String(err));
        process.exit(1);
      }
    });

  promptSets
    .command('get <uuid>')
    .description('Get prompt set details')
    .action(async (uuid: string) => {
      try {
        renderRedteamHeader();
        const service = await createPromptSetService();
        const ps = await service.getPromptSet(uuid);
        renderPromptSetDetail(ps);
        const info = await service.getPromptSetVersionInfo(uuid);
        renderVersionInfo(info);
      } catch (err) {
        renderError(err instanceof Error ? err.message : String(err));
        process.exit(1);
      }
    });

  promptSets
    .command('create')
    .description('Create a new prompt set')
    .requiredOption('--name <name>', 'Prompt set name')
    .option('--description <desc>', 'Prompt set description')
    .action(async (opts) => {
      try {
        renderRedteamHeader();
        const service = await createPromptSetService();
        const result = await service.createPromptSet(opts.name, opts.description);
        console.log(`  Prompt set created: ${result.uuid}\n`);
        console.log(`    Name: ${result.name}\n`);
      } catch (err) {
        renderError(err instanceof Error ? err.message : String(err));
        process.exit(1);
      }
    });

  promptSets
    .command('update <uuid>')
    .description('Update a prompt set')
    .option('--name <name>', 'New name')
    .option('--description <desc>', 'New description')
    .action(async (uuid: string, opts) => {
      try {
        renderRedteamHeader();
        const service = await createPromptSetService();
        const request: { name?: string; description?: string } = {};
        if (opts.name) request.name = opts.name;
        if (opts.description) request.description = opts.description;
        const result = await service.updatePromptSet(uuid, request);
        renderPromptSetDetail(result);
      } catch (err) {
        renderError(err instanceof Error ? err.message : String(err));
        process.exit(1);
      }
    });

  promptSets
    .command('archive <uuid>')
    .description('Archive a prompt set')
    .option('--unarchive', 'Unarchive instead')
    .action(async (uuid: string, opts) => {
      try {
        renderRedteamHeader();
        const service = await createPromptSetService();
        const archive = !opts.unarchive;
        await service.archivePromptSet(uuid, archive);
        console.log(`  Prompt set ${uuid} ${archive ? 'archived' : 'unarchived'}.\n`);
      } catch (err) {
        renderError(err instanceof Error ? err.message : String(err));
        process.exit(1);
      }
    });

  promptSets
    .command('download <uuid>')
    .description('Download CSV template for a prompt set')
    .option('--output <path>', 'Output file path')
    .action(async (uuid: string, opts) => {
      try {
        renderRedteamHeader();
        const service = await createPromptSetService();
        const csv = await service.downloadTemplate(uuid);
        const outPath = opts.output || `${uuid}-template.csv`;
        fs.writeFileSync(outPath, csv, 'utf-8');
        console.log(`  Template saved to ${outPath}\n`);
      } catch (err) {
        renderError(err instanceof Error ? err.message : String(err));
        process.exit(1);
      }
    });

  promptSets
    .command('upload <uuid> <file>')
    .description('Upload CSV prompts to a prompt set')
    .action(async (uuid: string, file: string) => {
      try {
        renderRedteamHeader();
        const service = await createPromptSetService();
        const content = fs.readFileSync(file);
        const blob = new Blob([content], { type: 'text/csv' });
        const result = await service.uploadPromptsCsv(uuid, blob);
        console.log(`  ${result.message}\n`);
      } catch (err) {
        renderError(err instanceof Error ? err.message : String(err));
        process.exit(1);
      }
    });

  // -----------------------------------------------------------------------
  // redteam prompts — individual prompt CRUD subcommands
  // -----------------------------------------------------------------------
  const prompts = redteam.command('prompts').description('Manage prompts within prompt sets');

  prompts
    .command('list <setUuid>')
    .description('List prompts in a prompt set')
    .option('--limit <n>', 'Max results', '50')
    .action(async (setUuid: string, opts) => {
      try {
        renderRedteamHeader();
        const service = await createPromptSetService();
        const list = await service.listPrompts(setUuid, {
          limit: Number.parseInt(opts.limit, 10),
        });
        renderPromptList(list);
      } catch (err) {
        renderError(err instanceof Error ? err.message : String(err));
        process.exit(1);
      }
    });

  prompts
    .command('get <setUuid> <promptUuid>')
    .description('Get prompt details')
    .action(async (setUuid: string, promptUuid: string) => {
      try {
        renderRedteamHeader();
        const service = await createPromptSetService();
        const prompt = await service.getPrompt(setUuid, promptUuid);
        renderPromptDetail(prompt);
      } catch (err) {
        renderError(err instanceof Error ? err.message : String(err));
        process.exit(1);
      }
    });

  prompts
    .command('add <setUuid>')
    .description('Add a prompt to a prompt set')
    .requiredOption('--prompt <text>', 'Prompt text')
    .option('--goal <text>', 'Prompt goal')
    .action(async (setUuid: string, opts) => {
      try {
        renderRedteamHeader();
        const service = await createPromptSetService();
        const result = await service.addPrompt(setUuid, opts.prompt, opts.goal);
        console.log(`  Prompt added: ${result.uuid}\n`);
      } catch (err) {
        renderError(err instanceof Error ? err.message : String(err));
        process.exit(1);
      }
    });

  prompts
    .command('update <setUuid> <promptUuid>')
    .description('Update a prompt')
    .option('--prompt <text>', 'New prompt text')
    .option('--goal <text>', 'New goal')
    .action(async (setUuid: string, promptUuid: string, opts) => {
      try {
        renderRedteamHeader();
        const service = await createPromptSetService();
        const request: { prompt?: string; goal?: string } = {};
        if (opts.prompt) request.prompt = opts.prompt;
        if (opts.goal) request.goal = opts.goal;
        const result = await service.updatePrompt(setUuid, promptUuid, request);
        renderPromptDetail(result);
      } catch (err) {
        renderError(err instanceof Error ? err.message : String(err));
        process.exit(1);
      }
    });

  prompts
    .command('delete <setUuid> <promptUuid>')
    .description('Delete a prompt')
    .action(async (setUuid: string, promptUuid: string) => {
      try {
        renderRedteamHeader();
        const service = await createPromptSetService();
        await service.deletePrompt(setUuid, promptUuid);
        console.log(`  Prompt ${promptUuid} deleted.\n`);
      } catch (err) {
        renderError(err instanceof Error ? err.message : String(err));
        process.exit(1);
      }
    });

  // -----------------------------------------------------------------------
  // redteam properties — property name/value management
  // -----------------------------------------------------------------------
  const properties = redteam.command('properties').description('Manage custom attack properties');

  properties
    .command('list')
    .description('List property names')
    .option('--output <format>', 'Output format: pretty, table, csv, json, yaml', 'pretty')
    .action(async (opts) => {
      try {
        const fmt = opts.output as OutputFormat;
        if (fmt === 'pretty') renderRedteamHeader();
        const service = await createPromptSetService();
        const names = await service.getPropertyNames();
        renderPropertyNames(names, fmt);
      } catch (err) {
        renderError(err instanceof Error ? err.message : String(err));
        process.exit(1);
      }
    });

  properties
    .command('create')
    .description('Create a property name')
    .requiredOption('--name <name>', 'Property name')
    .action(async (opts) => {
      try {
        renderRedteamHeader();
        const service = await createPromptSetService();
        const result = await service.createPropertyName(opts.name);
        console.log(`  Property created: ${result.name}\n`);
      } catch (err) {
        renderError(err instanceof Error ? err.message : String(err));
        process.exit(1);
      }
    });

  properties
    .command('values <name>')
    .description('List values for a property')
    .action(async (name: string) => {
      try {
        renderRedteamHeader();
        const service = await createPromptSetService();
        const values = await service.getPropertyValues(name);
        renderPropertyValues(values);
      } catch (err) {
        renderError(err instanceof Error ? err.message : String(err));
        process.exit(1);
      }
    });

  properties
    .command('add-value')
    .description('Create a property value')
    .requiredOption('--name <name>', 'Property name')
    .requiredOption('--value <value>', 'Property value')
    .action(async (opts) => {
      try {
        renderRedteamHeader();
        const service = await createPromptSetService();
        const result = await service.createPropertyValue(opts.name, opts.value);
        console.log(`  Value created: ${result.name}=${result.value}\n`);
      } catch (err) {
        renderError(err instanceof Error ? err.message : String(err));
        process.exit(1);
      }
    });

  // -----------------------------------------------------------------------
  // redteam report — view scan report
  // -----------------------------------------------------------------------
  redteam
    .command('report <jobId>')
    .description('View scan report')
    .option('--attacks', 'Include attack list', false)
    .option('--severity <level>', 'Filter attacks by severity')
    .option('--limit <n>', 'Max attacks to show', '20')
    .action(async (jobId: string, opts) => {
      try {
        renderRedteamHeader();
        const service = await createService();
        const job = await service.getScan(jobId);
        renderScanStatus(job);

        if (job.jobType === 'CUSTOM') {
          const report = await service.getCustomReport(jobId);
          renderCustomReport(report);
        } else {
          const report = await service.getStaticReport(jobId);
          renderStaticReport(report);
        }

        if (opts.attacks) {
          if (job.jobType === 'CUSTOM') {
            const attacks = await service.listCustomAttacks(jobId, {
              limit: Number.parseInt(opts.limit, 10),
            });
            renderCustomAttackList(attacks);
          } else {
            const attacks = await service.listAttacks(jobId, {
              severity: opts.severity,
              limit: Number.parseInt(opts.limit, 10),
            });
            renderAttackList(attacks);
          }
        }
      } catch (err) {
        renderError(err instanceof Error ? err.message : String(err));
        process.exit(1);
      }
    });

  // -----------------------------------------------------------------------
  // redteam scan — execute a red team scan
  // -----------------------------------------------------------------------
  redteam
    .command('scan')
    .description('Execute a red team scan against a target')
    .requiredOption('--target <uuid>', 'Target UUID')
    .requiredOption('--name <name>', 'Scan name')
    .option('--type <type>', 'Job type: STATIC, DYNAMIC, or CUSTOM', 'STATIC')
    .option('--categories <json>', 'Category filter JSON (STATIC scans)')
    .option('--prompt-sets <uuids>', 'Comma-separated prompt set UUIDs (CUSTOM scans)')
    .option('--no-wait', 'Submit scan without waiting for completion')
    .action(async (opts) => {
      try {
        renderRedteamHeader();
        const service = await createService();

        let categories: Record<string, unknown> | undefined;
        if (opts.categories) {
          categories = JSON.parse(opts.categories);
        }

        const customPromptSets = opts.promptSets
          ? (opts.promptSets as string).split(',').map((s: string) => s.trim())
          : undefined;

        console.log(`  Creating ${opts.type} scan "${opts.name}"...`);
        const job = await service.createScan({
          name: opts.name,
          targetUuid: opts.target,
          jobType: opts.type,
          categories,
          customPromptSets,
        });

        renderScanStatus(job);

        if (opts.wait !== false) {
          console.log('  Waiting for completion...\n');
          const completed = await service.waitForCompletion(job.uuid, (progress) =>
            renderScanProgress(progress),
          );
          console.log('\n');
          renderScanStatus(completed);
          console.log(`  Job ID: ${completed.uuid}`);
          console.log('  Run `airs redteam report <jobId>` to view results.\n');
        } else {
          console.log(`  Job ID: ${job.uuid}`);
          console.log('  Run `airs redteam status <jobId>` to check progress.\n');
        }
      } catch (err) {
        renderError(err instanceof Error ? err.message : String(err));
        process.exit(1);
      }
    });

  // -----------------------------------------------------------------------
  // redteam status — check scan status
  // -----------------------------------------------------------------------
  redteam
    .command('status <jobId>')
    .description('Check scan status')
    .action(async (jobId: string) => {
      try {
        renderRedteamHeader();
        const service = await createService();
        const job = await service.getScan(jobId);
        renderScanStatus(job);
      } catch (err) {
        renderError(err instanceof Error ? err.message : String(err));
        process.exit(1);
      }
    });

  // -----------------------------------------------------------------------
  // redteam targets — target CRUD subcommands
  // -----------------------------------------------------------------------
  const targets = redteam.command('targets').description('Manage red team targets');

  targets
    .command('list')
    .description('List configured red team targets')
    .option('--output <format>', 'Output format: pretty, table, csv, json, yaml', 'pretty')
    .action(async (opts) => {
      try {
        const fmt = opts.output as OutputFormat;
        if (fmt === 'pretty') renderRedteamHeader();
        const service = await createService();
        const list = await service.listTargets();
        renderTargetList(list, fmt);
      } catch (err) {
        renderError(err instanceof Error ? err.message : String(err));
        process.exit(1);
      }
    });

  targets
    .command('get <uuid>')
    .description('Get target details')
    .action(async (uuid: string) => {
      try {
        renderRedteamHeader();
        const service = await createService();
        const target = await service.getTarget(uuid);
        renderTargetDetail(target);
      } catch (err) {
        renderError(err instanceof Error ? err.message : String(err));
        process.exit(1);
      }
    });

  targets
    .command('create')
    .description('Create a new red team target')
    .requiredOption('--config <path>', 'JSON file with target configuration')
    .option('--validate', 'Validate target connection before saving')
    .action(async (opts) => {
      try {
        renderRedteamHeader();
        const service = await createService();
        const config = JSON.parse(fs.readFileSync(opts.config, 'utf-8'));
        const target = await service.createTarget(
          config,
          opts.validate ? { validate: true } : undefined,
        );
        console.log(`  Target created: ${target.uuid}\n`);
        renderTargetDetail(target);
      } catch (err) {
        renderError(err instanceof Error ? err.message : String(err));
        process.exit(1);
      }
    });

  targets
    .command('update <uuid>')
    .description('Update a red team target')
    .requiredOption('--config <path>', 'JSON file with target updates')
    .option('--validate', 'Validate target connection before saving')
    .action(async (uuid: string, opts) => {
      try {
        renderRedteamHeader();
        const service = await createService();
        const config = JSON.parse(fs.readFileSync(opts.config, 'utf-8'));
        const target = await service.updateTarget(
          uuid,
          config,
          opts.validate ? { validate: true } : undefined,
        );
        console.log(`  Target updated: ${target.uuid}\n`);
        renderTargetDetail(target);
      } catch (err) {
        renderError(err instanceof Error ? err.message : String(err));
        process.exit(1);
      }
    });

  targets
    .command('delete <uuid>')
    .description('Delete a red team target')
    .action(async (uuid: string) => {
      try {
        renderRedteamHeader();
        const service = await createService();
        await service.deleteTarget(uuid);
        console.log(`  Target ${uuid} deleted.\n`);
      } catch (err) {
        renderError(err instanceof Error ? err.message : String(err));
        process.exit(1);
      }
    });

  targets
    .command('probe')
    .description('Test target connection without saving')
    .requiredOption('--config <path>', 'JSON file with connection params')
    .action(async (opts) => {
      try {
        renderRedteamHeader();
        const service = await createService();
        const config = JSON.parse(fs.readFileSync(opts.config, 'utf-8'));
        const result = await service.probeTarget(config);
        console.log('  Probe result:');
        console.log(`    ${JSON.stringify(result, null, 2)}\n`);
      } catch (err) {
        renderError(err instanceof Error ? err.message : String(err));
        process.exit(1);
      }
    });

  targets
    .command('profile <uuid>')
    .description('View target profile')
    .action(async (uuid: string) => {
      try {
        renderRedteamHeader();
        const service = await createService();
        const profile = await service.getTargetProfile(uuid);
        console.log('  Target Profile:');
        console.log(`    ${JSON.stringify(profile, null, 2)}\n`);
      } catch (err) {
        renderError(err instanceof Error ? err.message : String(err));
        process.exit(1);
      }
    });

  targets
    .command('update-profile <uuid>')
    .description('Update target profile')
    .requiredOption('--config <path>', 'JSON file with profile updates')
    .action(async (uuid: string, opts) => {
      try {
        renderRedteamHeader();
        const service = await createService();
        const config = JSON.parse(fs.readFileSync(opts.config, 'utf-8'));
        const result = await service.updateTargetProfile(uuid, config);
        console.log('  Profile updated:');
        console.log(`    ${JSON.stringify(result, null, 2)}\n`);
      } catch (err) {
        renderError(err instanceof Error ? err.message : String(err));
        process.exit(1);
      }
    });
}
