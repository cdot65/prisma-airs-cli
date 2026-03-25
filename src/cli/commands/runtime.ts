import * as fs from 'node:fs';
import { readFile, writeFile } from 'node:fs/promises';
import chalk from 'chalk';
import type { Command } from 'commander';
import { SdkManagementService } from '../../airs/management.js';
import { SdkRuntimeService } from '../../airs/runtime.js';
import type { RuntimeScanResult, SecurityProfileInfo } from '../../airs/types.js';
import { loadConfig } from '../../config/loader.js';
import {
  buildProfileOverrides,
  buildProfileRequest,
  mergeProfilePolicy,
} from '../builders/profile-builder.js';
import { loadBulkScanState, saveBulkScanState } from '../bulk-scan-state.js';
import { parseInputFile } from '../parse-input.js';
import {
  OUTPUT_FORMATS,
  type OutputFormat,
  renderApiKeyDetail,
  renderApiKeyList,
  renderCustomerAppDetail,
  renderCustomerAppList,
  renderDeploymentProfileList,
  renderDlpProfileList,
  renderError,
  renderProfileDetail,
  renderProfileList,
  renderRuntimeConfigHeader,
  renderScanLogList,
  renderTopicDetail,
  renderTopicList,
} from '../renderer/index.js';
import { registerAuditCommand } from './audit.js';
import { registerGenerateCommand } from './generate.js';
import { registerListCommand } from './list.js';
import { registerReportCommand } from './report.js';
import { registerResumeCommand } from './resume.js';

function renderScanResult(result: RuntimeScanResult): void {
  const actionColor = result.action === 'block' ? chalk.red : chalk.green;
  console.log(chalk.bold('\n  Scan Result'));
  console.log(chalk.dim('  ─────────────────────────'));
  console.log(`  Action:    ${actionColor(result.action.toUpperCase())}`);
  console.log(`  Category:  ${result.category}`);
  console.log(`  Triggered: ${result.triggered ? chalk.red('yes') : chalk.green('no')}`);
  console.log(`  Scan ID:   ${chalk.dim(result.scanId)}`);
  console.log(`  Report ID: ${chalk.dim(result.reportId)}`);

  const flags = Object.entries(result.detections).filter(([, v]) => v);
  if (flags.length > 0) {
    console.log(chalk.bold('\n  Detections:'));
    for (const [key] of flags) {
      console.log(`    ${chalk.yellow('●')} ${key}`);
    }
  }
  console.log();
}

/** Create a management service from config. */
async function createMgmtService() {
  const config = await loadConfig();
  return new SdkManagementService({
    clientId: config.mgmtClientId,
    clientSecret: config.mgmtClientSecret,
    tsgId: config.mgmtTsgId,
    tokenEndpoint: config.mgmtTokenEndpoint,
  });
}

export function registerRuntimeCommand(program: Command): void {
  const runtime = program
    .command('runtime')
    .description('Runtime prompt scanning against AIRS profiles');

  // -----------------------------------------------------------------------
  // runtime api-keys — API key management subcommands
  // -----------------------------------------------------------------------
  const apiKeys = runtime.command('api-keys').description('Manage AIRS API keys');

  apiKeys
    .command('list')
    .description('List API keys')
    .option('--limit <n>', 'Max results', '100')
    .option('--output <format>', 'Output format: pretty, table, csv, json, yaml', 'pretty')
    .action(async (opts) => {
      try {
        const fmt = opts.output as OutputFormat;
        if (fmt === 'pretty') renderRuntimeConfigHeader();
        const service = await createMgmtService();
        const result = await service.listApiKeys({
          limit: Number.parseInt(opts.limit, 10),
        });
        renderApiKeyList(result.apiKeys, fmt);
      } catch (err) {
        renderError(err instanceof Error ? err.message : String(err));
        process.exit(1);
      }
    });

  apiKeys
    .command('create')
    .description('Create a new API key')
    .requiredOption('--config <path>', 'JSON file with API key configuration')
    .action(async (opts) => {
      try {
        renderRuntimeConfigHeader();
        const service = await createMgmtService();
        const config = JSON.parse(fs.readFileSync(opts.config, 'utf-8'));
        const key = await service.createApiKey(config);
        console.log(`  API key created: ${key.id}\n`);
        renderApiKeyDetail(key);
      } catch (err) {
        renderError(err instanceof Error ? err.message : String(err));
        process.exit(1);
      }
    });

  apiKeys
    .command('regenerate <apiKeyId>')
    .description('Regenerate an API key')
    .requiredOption('--interval <n>', 'Rotation time interval')
    .requiredOption('--unit <unit>', 'Rotation time unit (hours, days, months)')
    .option('--updated-by <email>', 'Email of user performing regeneration')
    .action(async (apiKeyId: string, opts) => {
      try {
        renderRuntimeConfigHeader();
        const service = await createMgmtService();
        const request: Record<string, unknown> = {
          rotation_time_interval: Number.parseInt(opts.interval, 10),
          rotation_time_unit: opts.unit,
        };
        if (opts.updatedBy) request.updated_by = opts.updatedBy;
        const key = await service.regenerateApiKey(apiKeyId, request);
        console.log(`  API key regenerated: ${key.id}\n`);
        renderApiKeyDetail(key);
      } catch (err) {
        renderError(err instanceof Error ? err.message : String(err));
        process.exit(1);
      }
    });

  apiKeys
    .command('delete <apiKeyName>')
    .description('Delete an API key')
    .requiredOption('--updated-by <email>', 'Email of user performing deletion')
    .action(async (apiKeyName: string, opts) => {
      try {
        renderRuntimeConfigHeader();
        const service = await createMgmtService();
        const result = await service.deleteApiKey(apiKeyName, opts.updatedBy);
        console.log(`  ${result.message}\n`);
      } catch (err) {
        renderError(err instanceof Error ? err.message : String(err));
        process.exit(1);
      }
    });

  // -----------------------------------------------------------------------
  // runtime bulk-scan — async bulk scanning
  // -----------------------------------------------------------------------
  runtime
    .command('bulk-scan')
    .description('Scan multiple prompts via the async AIRS API')
    .requiredOption('--profile <name>', 'Security profile name')
    .requiredOption(
      '--input <file>',
      'Input file — .csv (extracts prompt column) or .txt (one per line)',
    )
    .option('--output <file>', 'Output CSV file path')
    .option('--session-id <id>', 'Session ID for grouping scans in AIRS dashboard')
    .action(async (opts) => {
      try {
        const config = await loadConfig({});
        if (!config.airsApiKey) {
          renderError('PANW_AI_SEC_API_KEY is required');
          process.exit(1);
        }

        const raw = await readFile(opts.input, 'utf-8');
        const prompts = parseInputFile(raw, opts.input);

        if (prompts.length === 0) {
          renderError('No prompts found in input file');
          process.exit(1);
        }

        const sessionId = opts.sessionId ?? `prisma-airs-cli-bulk-${Date.now().toString(36)}`;

        const service = new SdkRuntimeService(config.airsApiKey);
        console.log(chalk.bold.cyan('\n  Prisma AIRS Bulk Scan'));
        console.log(chalk.dim(`  Profile:  ${opts.profile}`));
        console.log(chalk.dim(`  Session:  ${sessionId}`));
        console.log(chalk.dim(`  Prompts:  ${prompts.length}`));
        console.log(chalk.dim(`  Batches:  ${Math.ceil(prompts.length / 5)}\n`));

        console.log(chalk.dim('  Submitting async scans...'));
        const scanIds = await service.submitBulkScan(opts.profile, prompts, sessionId);

        const stateDir = config.dataDir.replace(/\/runs$/, '/bulk-scans');
        const statePath = await saveBulkScanState(
          { scanIds, profile: opts.profile, promptCount: prompts.length, sessionId },
          stateDir,
        );
        console.log(chalk.dim(`  Scan IDs saved: ${statePath}`));
        console.log(chalk.dim(`  Submitted ${scanIds.length} batch(es), polling for results...`));

        const results = await service.pollResults(scanIds, undefined, {
          onRetry: (attempt, delayMs) => {
            console.log(
              chalk.yellow(
                `  ⚠ Rate limited — retry ${attempt} in ${(delayMs / 1000).toFixed(0)}s...`,
              ),
            );
          },
        });

        // Attach prompts to results
        for (let i = 0; i < results.length && i < prompts.length; i++) {
          results[i].prompt = prompts[i];
        }

        const outputPath = opts.output ?? `${opts.profile.replace(/\s+/g, '-')}-bulk-scan.csv`;
        const csv = SdkRuntimeService.formatResultsCsv(results);
        await writeFile(outputPath, csv, 'utf-8');

        const blocked = results.filter((r) => r.action === 'block').length;
        const allowed = results.filter((r) => r.action === 'allow').length;

        console.log(chalk.bold('\n  Bulk Scan Complete'));
        console.log(chalk.dim('  ─────────────────────────'));
        console.log(`  Total:   ${results.length}`);
        console.log(`  Blocked: ${chalk.red(String(blocked))}`);
        console.log(`  Allowed: ${chalk.green(String(allowed))}`);
        console.log(`  Output:  ${chalk.cyan(outputPath)}\n`);
      } catch (err) {
        renderError(err instanceof Error ? err.message : String(err));
        process.exit(1);
      }
    });

  // -----------------------------------------------------------------------
  // runtime customer-apps — customer app management subcommands
  // -----------------------------------------------------------------------
  const customerApps = runtime.command('customer-apps').description('Manage AIRS customer apps');

  customerApps
    .command('list')
    .description('List customer apps')
    .option('--limit <n>', 'Max results', '100')
    .option('--output <format>', 'Output format: pretty, table, csv, json, yaml', 'pretty')
    .action(async (opts) => {
      try {
        const fmt = opts.output as OutputFormat;
        if (fmt === 'pretty') renderRuntimeConfigHeader();
        const service = await createMgmtService();
        const result = await service.listCustomerApps({
          limit: Number.parseInt(opts.limit, 10),
        });
        renderCustomerAppList(result.apps, fmt);
      } catch (err) {
        renderError(err instanceof Error ? err.message : String(err));
        process.exit(1);
      }
    });

  customerApps
    .command('get <appName>')
    .description('Get customer app details')
    .action(async (appName: string) => {
      try {
        renderRuntimeConfigHeader();
        const service = await createMgmtService();
        const app = await service.getCustomerApp(appName);
        renderCustomerAppDetail(app);
      } catch (err) {
        renderError(err instanceof Error ? err.message : String(err));
        process.exit(1);
      }
    });

  customerApps
    .command('update <appId>')
    .description('Update a customer app')
    .requiredOption('--config <path>', 'JSON file with app updates')
    .action(async (appId: string, opts) => {
      try {
        renderRuntimeConfigHeader();
        const service = await createMgmtService();
        const config = JSON.parse(fs.readFileSync(opts.config, 'utf-8'));
        const app = await service.updateCustomerApp(appId, config);
        console.log(`  Customer app updated: ${app.name}\n`);
        renderCustomerAppDetail(app);
      } catch (err) {
        renderError(err instanceof Error ? err.message : String(err));
        process.exit(1);
      }
    });

  customerApps
    .command('delete <appName>')
    .description('Delete a customer app')
    .requiredOption('--updated-by <email>', 'Email of user performing deletion')
    .action(async (appName: string, opts) => {
      try {
        renderRuntimeConfigHeader();
        const service = await createMgmtService();
        const app = await service.deleteCustomerApp(appName, opts.updatedBy);
        console.log(`  Customer app "${app.name}" deleted.\n`);
      } catch (err) {
        renderError(err instanceof Error ? err.message : String(err));
        process.exit(1);
      }
    });

  // -----------------------------------------------------------------------
  // runtime deployment-profiles — read-only listing
  // -----------------------------------------------------------------------
  const deploymentProfiles = runtime
    .command('deployment-profiles')
    .description('List AIRS deployment profiles');

  deploymentProfiles
    .command('list')
    .description('List deployment profiles')
    .option('--unactivated', 'Include unactivated profiles')
    .option('--output <format>', 'Output format: pretty, table, csv, json, yaml', 'pretty')
    .action(async (opts) => {
      try {
        const fmt = opts.output as OutputFormat;
        if (fmt === 'pretty') renderRuntimeConfigHeader();
        const service = await createMgmtService();
        const profiles = await service.listDeploymentProfiles({
          unactivated: opts.unactivated,
        });
        renderDeploymentProfileList(profiles, fmt);
      } catch (err) {
        renderError(err instanceof Error ? err.message : String(err));
        process.exit(1);
      }
    });

  // -----------------------------------------------------------------------
  // runtime dlp-profiles — read-only listing
  // -----------------------------------------------------------------------
  const dlpProfiles = runtime.command('dlp-profiles').description('List AIRS DLP profiles');

  dlpProfiles
    .command('list')
    .description('List DLP profiles')
    .option('--output <format>', 'Output format: pretty, table, csv, json, yaml', 'pretty')
    .action(async (opts) => {
      try {
        const fmt = opts.output as OutputFormat;
        if (fmt === 'pretty') renderRuntimeConfigHeader();
        const service = await createMgmtService();
        const profiles = await service.listDlpProfiles();
        renderDlpProfileList(profiles, fmt);
      } catch (err) {
        renderError(err instanceof Error ? err.message : String(err));
        process.exit(1);
      }
    });

  // -----------------------------------------------------------------------
  // runtime profiles — security profile CRUD subcommands
  // -----------------------------------------------------------------------
  const profiles = runtime.command('profiles').description('Manage AIRS security profiles');

  profiles
    .command('list')
    .description('List security profiles')
    .option('--limit <n>', 'Max results', '100')
    .option('--offset <n>', 'Starting offset', '0')
    .option('--output <format>', 'Output format: pretty, table, csv, json, yaml', 'pretty')
    .action(async (opts) => {
      try {
        const fmt = opts.output as OutputFormat;
        if (!OUTPUT_FORMATS.includes(fmt)) {
          renderError(`Invalid output format "${fmt}". Valid: ${OUTPUT_FORMATS.join(', ')}`);
          process.exit(1);
        }
        if (fmt === 'pretty') renderRuntimeConfigHeader();
        const service = await createMgmtService();
        const result = await service.listProfiles({
          limit: Number.parseInt(opts.limit, 10),
          offset: Number.parseInt(opts.offset, 10),
        });
        renderProfileList(result.profiles, fmt);
        if (fmt === 'pretty' && result.nextOffset != null) {
          console.log(chalk.dim(`  Next offset: ${result.nextOffset}\n`));
        }
      } catch (err) {
        renderError(err instanceof Error ? err.message : String(err));
        process.exit(1);
      }
    });

  profiles
    .command('get <nameOrId>')
    .description('Get a security profile by name or UUID')
    .option('--output <format>', 'Output format: pretty, json, yaml', 'pretty')
    .action(async (nameOrId: string, opts) => {
      try {
        const fmt = opts.output as OutputFormat;
        if (fmt !== 'pretty' && fmt !== 'json' && fmt !== 'yaml') {
          renderError(`Invalid output format "${fmt}". Valid: pretty, json, yaml`);
          process.exit(1);
        }
        if (fmt === 'pretty') renderRuntimeConfigHeader();
        const service = await createMgmtService();
        const isUuid = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(
          nameOrId,
        );
        const profile = isUuid
          ? await service.getProfile(nameOrId)
          : await service.getProfileByName(nameOrId);
        if (fmt === 'json') {
          console.log(JSON.stringify(profile, null, 2));
        } else if (fmt === 'yaml') {
          const lines = [`profileId: ${profile.profileId}`, `profileName: ${profile.profileName}`];
          if (profile.revision != null) lines.push(`revision: ${profile.revision}`);
          if (profile.active != null) lines.push(`active: ${profile.active}`);
          if (profile.createdBy) lines.push(`createdBy: ${profile.createdBy}`);
          if (profile.updatedBy) lines.push(`updatedBy: ${profile.updatedBy}`);
          if (profile.lastModifiedTs) lines.push(`lastModifiedTs: ${profile.lastModifiedTs}`);
          if (profile.policy) lines.push(`policy: ${JSON.stringify(profile.policy, null, 2)}`);
          console.log(lines.join('\n'));
        } else {
          renderProfileDetail(profile);
        }
      } catch (err) {
        renderError(err instanceof Error ? err.message : String(err));
        process.exit(1);
      }
    });

  profiles
    .command('create')
    .description('Create a new security profile')
    .requiredOption('--name <name>', 'Profile name')
    .option('--no-active', 'Create profile as inactive')
    .option('--prompt-injection <action>', 'Prompt injection action (block/allow/alert)')
    .option('--toxic-content <action>', 'Toxic content action (e.g. "high:block, moderate:block")')
    .option('--contextual-grounding <action>', 'Contextual grounding action (block/allow/alert)')
    .option('--malicious-code <action>', 'Malicious code protection action (block/allow/alert)')
    .option('--url-action <action>', 'URL detected action (block/allow/alert)')
    .option('--allow-url-categories <list>', 'Comma-separated URL categories to allow')
    .option('--block-url-categories <list>', 'Comma-separated URL categories to block')
    .option('--alert-url-categories <list>', 'Comma-separated URL categories to alert')
    .option('--agent-security <action>', 'Agent security action (block/allow/alert)')
    .option('--dlp-action <action>', 'Data leak detection action (block/allow/alert)')
    .option('--dlp-profiles <list>', 'Comma-separated DLP profile names')
    .option('--mask-data-inline', 'Mask detected data inline')
    .option('--db-security-create <action>', 'Database create action (block/allow/alert)')
    .option('--db-security-read <action>', 'Database read action (block/allow/alert)')
    .option('--db-security-update <action>', 'Database update action (block/allow/alert)')
    .option('--db-security-delete <action>', 'Database delete action (block/allow/alert)')
    .option('--inline-timeout-action <action>', 'Inline timeout action (block/allow)')
    .option('--max-inline-latency <n>', 'Max inline latency in seconds', Number.parseFloat)
    .option('--mask-data-in-storage', 'Mask data in storage')
    .option('--config <path>', 'JSON file with profile configuration (legacy)')
    .action(async (opts) => {
      try {
        renderRuntimeConfigHeader();
        const service = await createMgmtService();

        let profile: SecurityProfileInfo;
        if (opts.config) {
          // Legacy JSON file path
          const config = JSON.parse(fs.readFileSync(opts.config, 'utf-8'));
          profile = await service.createProfile(config);
        } else {
          const request = buildProfileRequest({
            name: opts.name,
            active: opts.active,
            promptInjection: opts.promptInjection,
            toxicContent: opts.toxicContent,
            contextualGrounding: opts.contextualGrounding,
            maliciousCode: opts.maliciousCode,
            urlAction: opts.urlAction,
            allowUrlCategories: opts.allowUrlCategories,
            blockUrlCategories: opts.blockUrlCategories,
            alertUrlCategories: opts.alertUrlCategories,
            agentSecurity: opts.agentSecurity,
            dlpAction: opts.dlpAction,
            dlpProfiles: opts.dlpProfiles,
            maskDataInline: opts.maskDataInline,
            dbSecurityCreate: opts.dbSecurityCreate,
            dbSecurityRead: opts.dbSecurityRead,
            dbSecurityUpdate: opts.dbSecurityUpdate,
            dbSecurityDelete: opts.dbSecurityDelete,
            inlineTimeoutAction: opts.inlineTimeoutAction,
            maxInlineLatency: opts.maxInlineLatency,
            maskDataInStorage: opts.maskDataInStorage,
          });
          profile = await service.createProfile(request);
        }

        console.log(`  Profile created: ${profile.profileId}\n`);
        renderProfileDetail(profile);
      } catch (err) {
        const msg = err instanceof Error ? err.message : String(err);
        if (msg.includes('409')) {
          renderError(`Profile "${opts.name}" already exists. Use 'profiles update' to modify it.`);
        } else {
          renderError(msg);
        }
        process.exit(1);
      }
    });

  profiles
    .command('update <nameOrId>')
    .description('Update a security profile by name or UUID')
    .option('--name <name>', 'Update profile name')
    .option('--no-active', 'Set profile as inactive')
    .option('--active', 'Set profile as active')
    .option('--prompt-injection <action>', 'Prompt injection action (block/allow/alert)')
    .option('--toxic-content <action>', 'Toxic content action (e.g. "high:block, moderate:block")')
    .option('--contextual-grounding <action>', 'Contextual grounding action (block/allow/alert)')
    .option('--malicious-code <action>', 'Malicious code protection action (block/allow/alert)')
    .option('--url-action <action>', 'URL detected action (block/allow/alert)')
    .option('--allow-url-categories <list>', 'Comma-separated URL categories to allow')
    .option('--block-url-categories <list>', 'Comma-separated URL categories to block')
    .option('--alert-url-categories <list>', 'Comma-separated URL categories to alert')
    .option('--agent-security <action>', 'Agent security action (block/allow/alert)')
    .option('--dlp-action <action>', 'Data leak detection action (block/allow/alert)')
    .option('--dlp-profiles <list>', 'Comma-separated DLP profile names')
    .option('--mask-data-inline', 'Mask detected data inline')
    .option('--db-security-create <action>', 'Database create action (block/allow/alert)')
    .option('--db-security-read <action>', 'Database read action (block/allow/alert)')
    .option('--db-security-update <action>', 'Database update action (block/allow/alert)')
    .option('--db-security-delete <action>', 'Database delete action (block/allow/alert)')
    .option('--inline-timeout-action <action>', 'Inline timeout action (block/allow)')
    .option('--max-inline-latency <n>', 'Max inline latency in seconds', Number.parseFloat)
    .option('--mask-data-in-storage', 'Mask data in storage')
    .option('--config <path>', 'JSON file with profile updates (legacy)')
    .action(async (nameOrId: string, opts) => {
      try {
        renderRuntimeConfigHeader();
        const service = await createMgmtService();
        const isUuid = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(
          nameOrId,
        );
        const resolved = isUuid
          ? await service.getProfile(nameOrId)
          : await service.getProfileByName(nameOrId);
        const profileId = resolved.profileId;

        let profile: SecurityProfileInfo;
        if (opts.config) {
          // Legacy JSON file path
          const config = JSON.parse(fs.readFileSync(opts.config, 'utf-8'));
          profile = await service.updateProfile(profileId, config);
        } else {
          // Read-modify-write: fetch current profile, merge flags, PUT full payload
          const current = resolved;
          const overrides = buildProfileOverrides({
            promptInjection: opts.promptInjection,
            toxicContent: opts.toxicContent,
            contextualGrounding: opts.contextualGrounding,
            maliciousCode: opts.maliciousCode,
            urlAction: opts.urlAction,
            allowUrlCategories: opts.allowUrlCategories,
            blockUrlCategories: opts.blockUrlCategories,
            alertUrlCategories: opts.alertUrlCategories,
            agentSecurity: opts.agentSecurity,
            dlpAction: opts.dlpAction,
            dlpProfiles: opts.dlpProfiles,
            maskDataInline: opts.maskDataInline,
            dbSecurityCreate: opts.dbSecurityCreate,
            dbSecurityRead: opts.dbSecurityRead,
            dbSecurityUpdate: opts.dbSecurityUpdate,
            dbSecurityDelete: opts.dbSecurityDelete,
            inlineTimeoutAction: opts.inlineTimeoutAction,
            maxInlineLatency: opts.maxInlineLatency,
            maskDataInStorage: opts.maskDataInStorage,
          });
          const mergedPolicy = mergeProfilePolicy(current.policy, overrides);

          profile = await service.updateProfile(profileId, {
            profile_name: opts.name ?? current.profileName,
            active: opts.active ?? current.active ?? true,
            policy: mergedPolicy,
          });
        }

        console.log(`  Profile updated: ${profile.profileId}\n`);
        renderProfileDetail(profile);
      } catch (err) {
        renderError(err instanceof Error ? err.message : String(err));
        process.exit(1);
      }
    });

  profiles
    .command('delete <nameOrId>')
    .description('Delete a security profile by name or UUID')
    .option('--force', 'Force delete (removes from referencing policies)')
    .option('--updated-by <email>', 'Email of user performing force deletion')
    .action(async (nameOrId: string, opts) => {
      try {
        renderRuntimeConfigHeader();
        const service = await createMgmtService();
        const isUuid = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(
          nameOrId,
        );
        let profileId = nameOrId;
        let profileName = nameOrId;
        if (!isUuid) {
          const profile = await service.getProfileByName(nameOrId);
          profileId = profile.profileId;
          profileName = profile.profileName;
        }
        if (opts.force) {
          if (!opts.updatedBy) {
            renderError('--updated-by <email> is required with --force');
            process.exit(1);
          }
          await service.forceDeleteProfile(profileId, opts.updatedBy);
        } else {
          await service.deleteProfile(profileId);
        }
        console.log(`  Profile deleted: ${profileName} (${profileId})\n`);
      } catch (err) {
        renderError(err instanceof Error ? err.message : String(err));
        process.exit(1);
      }
    });

  // Register audit under profiles
  registerAuditCommand(profiles);

  // -----------------------------------------------------------------------
  // runtime resume-poll — resume polling for bulk scans
  // -----------------------------------------------------------------------
  runtime
    .command('resume-poll <stateFile>')
    .description('Resume polling for a previously submitted bulk scan')
    .option('--output <file>', 'Output CSV file path')
    .action(async (stateFile: string, opts) => {
      try {
        const config = await loadConfig({});
        if (!config.airsApiKey) {
          renderError('PANW_AI_SEC_API_KEY is required');
          process.exit(1);
        }

        const state = await loadBulkScanState(stateFile);
        const service = new SdkRuntimeService(config.airsApiKey);

        console.log(chalk.bold.cyan('\n  Prisma AIRS Resume Poll'));
        console.log(chalk.dim(`  Profile:  ${state.profile}`));
        console.log(chalk.dim(`  Scan IDs: ${state.scanIds.length}`));
        console.log(chalk.dim(`  Prompts:  ${state.promptCount}\n`));

        console.log(chalk.dim('  Polling for results...'));
        const results = await service.pollResults(state.scanIds, undefined, {
          onRetry: (attempt, delayMs) => {
            console.log(
              chalk.yellow(
                `  ⚠ Rate limited — retry ${attempt} in ${(delayMs / 1000).toFixed(0)}s...`,
              ),
            );
          },
        });

        const outputPath = opts.output ?? `${state.profile.replace(/\s+/g, '-')}-bulk-scan.csv`;
        const csv = SdkRuntimeService.formatResultsCsv(results);
        await writeFile(outputPath, csv, 'utf-8');

        const blocked = results.filter((r) => r.action === 'block').length;
        const allowed = results.filter((r) => r.action === 'allow').length;

        console.log(chalk.bold('\n  Resume Poll Complete'));
        console.log(chalk.dim('  ─────────────────────────'));
        console.log(`  Total:   ${results.length}`);
        console.log(`  Blocked: ${chalk.red(String(blocked))}`);
        console.log(`  Allowed: ${chalk.green(String(allowed))}`);
        console.log(`  Output:  ${chalk.cyan(outputPath)}\n`);
      } catch (err) {
        renderError(err instanceof Error ? err.message : String(err));
        process.exit(1);
      }
    });

  // -----------------------------------------------------------------------
  // runtime scan — single prompt scanning
  // -----------------------------------------------------------------------
  runtime
    .command('scan <prompt>')
    .description('Scan a single prompt against an AIRS security profile')
    .requiredOption('--profile <name>', 'Security profile name')
    .option('--response <text>', 'Response text to scan alongside the prompt')
    .action(async (prompt: string, opts) => {
      try {
        const config = await loadConfig({});
        if (!config.airsApiKey) {
          renderError('PANW_AI_SEC_API_KEY is required');
          process.exit(1);
        }

        const service = new SdkRuntimeService(config.airsApiKey);
        console.log(chalk.bold.cyan('\n  Prisma AIRS Runtime Scan'));
        console.log(chalk.dim(`  Profile: ${opts.profile}`));
        console.log(
          chalk.dim(`  Prompt:  "${prompt.slice(0, 80)}${prompt.length > 80 ? '...' : ''}"`),
        );

        const result = await service.scanPrompt(opts.profile, prompt, opts.response);
        renderScanResult(result);
      } catch (err) {
        renderError(err instanceof Error ? err.message : String(err));
        process.exit(1);
      }
    });

  // -----------------------------------------------------------------------
  // runtime scan-logs — scan log query
  // -----------------------------------------------------------------------
  const scanLogs = runtime.command('scan-logs').description('Query AIRS scan logs');

  scanLogs
    .command('query')
    .description('Query scan logs')
    .requiredOption('--interval <n>', 'Time interval')
    .requiredOption('--unit <unit>', 'Time unit (hours)')
    .option('--filter <filter>', 'Filter: all, benign, threat', 'all')
    .option('--page <n>', 'Page number', '1')
    .option('--page-size <n>', 'Page size', '50')
    .option('--output <format>', 'Output format: pretty, table, csv, json, yaml', 'pretty')
    .action(async (opts) => {
      try {
        const fmt = opts.output as OutputFormat;
        if (fmt === 'pretty') renderRuntimeConfigHeader();
        const service = await createMgmtService();
        const result = await service.queryScanLogs({
          timeInterval: Number.parseInt(opts.interval, 10),
          timeUnit: opts.unit,
          pageNumber: Number.parseInt(opts.page, 10),
          pageSize: Number.parseInt(opts.pageSize, 10),
          filter: opts.filter,
        });
        renderScanLogList(result.results, result.pageToken, fmt);
      } catch (err) {
        renderError(err instanceof Error ? err.message : String(err));
        process.exit(1);
      }
    });

  // -----------------------------------------------------------------------
  // runtime topics — custom topic CRUD + guardrail generation subcommands
  // -----------------------------------------------------------------------
  const topics = runtime
    .command('topics')
    .description('Manage AIRS custom topics and guardrail generation');

  topics
    .command('list')
    .description('List custom topics')
    .option('--limit <n>', 'Max results', '100')
    .option('--offset <n>', 'Starting offset', '0')
    .option('--output <format>', 'Output format: pretty, table, csv, json, yaml', 'pretty')
    .action(async (opts) => {
      try {
        const fmt = opts.output as OutputFormat;
        if (fmt === 'pretty') renderRuntimeConfigHeader();
        const service = await createMgmtService();
        const allTopics = await service.listTopics();
        // Client-side pagination since SDK returns all
        const offset = Number.parseInt(opts.offset, 10);
        const limit = Number.parseInt(opts.limit, 10);
        const page = allTopics.slice(offset, offset + limit);
        renderTopicList(page, fmt);
        if (fmt === 'pretty' && offset + limit < allTopics.length) {
          console.log(chalk.dim(`  Showing ${page.length} of ${allTopics.length} topics\n`));
        }
      } catch (err) {
        renderError(err instanceof Error ? err.message : String(err));
        process.exit(1);
      }
    });

  topics
    .command('create')
    .description('Create a new custom topic')
    .requiredOption('--config <path>', 'JSON file with topic configuration')
    .action(async (opts) => {
      try {
        renderRuntimeConfigHeader();
        const service = await createMgmtService();
        const config = JSON.parse(fs.readFileSync(opts.config, 'utf-8'));
        const topic = await service.createTopic(config);
        console.log(`  Topic created: ${topic.topic_id}\n`);
        renderTopicDetail(topic);
      } catch (err) {
        renderError(err instanceof Error ? err.message : String(err));
        process.exit(1);
      }
    });

  topics
    .command('update <topicId>')
    .description('Update a custom topic')
    .requiredOption('--config <path>', 'JSON file with topic updates')
    .action(async (topicId: string, opts) => {
      try {
        renderRuntimeConfigHeader();
        const service = await createMgmtService();
        const config = JSON.parse(fs.readFileSync(opts.config, 'utf-8'));
        const topic = await service.updateTopic(topicId, config);
        console.log(`  Topic updated: ${topic.topic_id}\n`);
        renderTopicDetail(topic);
      } catch (err) {
        renderError(err instanceof Error ? err.message : String(err));
        process.exit(1);
      }
    });

  topics
    .command('delete <topicId>')
    .description('Delete a custom topic')
    .option('--force', 'Force delete (removes from all referencing profiles)')
    .option('--updated-by <email>', 'Email of user performing force deletion')
    .action(async (topicId: string, opts) => {
      try {
        renderRuntimeConfigHeader();
        const service = await createMgmtService();
        if (opts.force) {
          const result = await service.forceDeleteTopic(topicId, opts.updatedBy);
          console.log(`  ${result.message}\n`);
        } else {
          await service.deleteTopic(topicId);
          console.log(`  Topic ${topicId} deleted.\n`);
        }
      } catch (err) {
        renderError(err instanceof Error ? err.message : String(err));
        process.exit(1);
      }
    });

  // Register guardrail generation commands under topics
  registerGenerateCommand(topics);
  registerResumeCommand(topics);
  registerReportCommand(topics);
  registerListCommand(topics, 'runs');
}
