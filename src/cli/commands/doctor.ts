import { randomUUID } from 'node:crypto';
import { readFile } from 'node:fs/promises';
import { init, Scanner } from '@cdot65/prisma-airs-sdk';
import type { Command } from 'commander';
import { aiGatewayGrantHint, SdkAiGatewayService } from '../../airs/aigateway.js';
import { SdkManagementService } from '../../airs/management.js';
import {
  aiGatewayClientOptions,
  managementClientOptions,
  runtimeInitOptions,
} from '../../config/client-options.js';
import {
  MANAGEMENT_CREDENTIAL_KEYS,
  SCANNER_CREDENTIAL_KEYS,
  settingRemedy,
} from '../../config/credentials.js';
import { ignoredEnvironment, SDK_DIAGNOSTIC_ENV_VARS } from '../../config/env.js';
import {
  type ConfigContext,
  type ConfigEntry,
  inspectConfig,
  loadConfig,
  resolveConfigContext,
} from '../../config/loader.js';
import { type Config, ConfigSchema, RETIRED_CONFIG_KEYS } from '../../config/schema.js';
import { tenantStorePath } from '../../config/tenants.js';
import { examples } from '../examples.js';
import { type BulletKind, formatOutput, resolveOutput, ui } from '../renderer/index.js';

/**
 * `skip` marks a check that could not run because an optional prerequisite is
 * absent (no scanner key, a failed earlier check). It never affects the exit code.
 */
export type DoctorStatus = 'pass' | 'warn' | 'fail' | 'skip';

export interface DoctorCheck {
  name: string;
  status: DoctorStatus;
  detail: string;
  hint?: string;
}

/** Time box for each network check. */
export const DOCTOR_TIMEOUT_MS = 5000;

export const SUPPORTED_NODE_VERSIONS = '^20.17.0 || ^22.13.0 || >=23.5.0';

/** Check names in report order; also the order of `runDoctor()` results. */
export const DOCTOR_CHECK_NAMES = [
  'Node.js version',
  'Tenant',
  'Config file',
  'Environment',
  'Scanner credentials',
  'Management credentials',
  'Scanner API',
  'Management OAuth',
  'AI Gateway API',
] as const;

function plural(count: number, noun: string): string {
  return `${count} ${noun}${count === 1 ? '' : 's'}`;
}

function errMessage(err: unknown): string {
  return err instanceof Error ? err.message : String(err);
}

function notEvaluated(name: string): DoctorCheck {
  return { name, status: 'skip', detail: 'not evaluated — fix the failed checks above first' };
}

// ---------------------------------------------------------------------------
// Local checks
// ---------------------------------------------------------------------------

/** Check 1: the runtime satisfies the production dependency engine intersection. */
export function checkNodeVersion(version: string = process.version): DoctorCheck {
  const match = /^v?(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/.exec(version);
  const [major, minor, patch] = match ? match.slice(1).map(Number) : [];
  const supported =
    [major, minor, patch].every(Number.isSafeInteger) &&
    ((major === 20 && minor >= 17) ||
      (major === 22 && minor >= 13) ||
      (major === 23 && minor >= 5) ||
      major >= 24);
  if (supported) {
    return {
      name: 'Node.js version',
      status: 'pass',
      detail: `${version} (${SUPPORTED_NODE_VERSIONS} required)`,
    };
  }
  return {
    name: 'Node.js version',
    status: 'fail',
    detail: `${version} does not satisfy ${SUPPORTED_NODE_VERSIONS}`,
    hint: 'Use a supported Node runtime: 20.17+, 22.13+, or 24+ (e.g. via nvm or your package manager)',
  };
}

/** Check 2: which tenant every other command will use. Nothing works without one. */
export function checkTenant(context: ConfigContext): DoctorCheck {
  const name = 'Tenant';
  if (context.selection === 'tenant') {
    return {
      name,
      status: 'pass',
      detail: `${context.tenant.name} (TSG ${context.tenant.tsgId}) selected in ${context.registryPath}`,
    };
  }
  if (context.selection === 'explicit') {
    return { name, status: 'pass', detail: `explicit config path — ${context.path}` };
  }
  return {
    name,
    status: 'fail',
    detail: context.registered.length
      ? `no tenant selected (registered: ${context.registered.join(', ')})`
      : `no tenants registered in ${context.registryPath}`,
    hint: context.registered.length
      ? "Run 'airs tenant switch <name>'"
      : "Run 'airs tenant create <name>' (prompts for TSG ID, client ID and secret), then 'airs tenant switch <name>'",
  };
}

/** Tenant check when the registry itself cannot be read; nothing after it can be trusted. */
export function tenantRegistryFailure(err: unknown, registryPath: string): DoctorCheck {
  return {
    name: 'Tenant',
    status: 'fail',
    detail: errMessage(err),
    hint: `Restore or remove ${registryPath}; PRISMA_AIRS_CONFIG_PATH bypasses the registry meanwhile`,
  };
}

/**
 * Check 3: the selected config file. A tenant file must parse, validate, and carry
 * the pinned TSG. Keys from retired releases are reported as ignored.
 */
export async function checkConfigFile(context: ConfigContext): Promise<DoctorCheck> {
  const name = 'Config file';
  if (context.selection === 'none') return notEvaluated(name);
  const { path } = context;
  let raw: string;
  try {
    raw = await readFile(path, 'utf-8');
  } catch {
    return {
      name,
      status: 'fail',
      detail: `not found at ${path}`,
      hint:
        context.selection === 'tenant'
          ? `Restore the file, or register another with 'airs tenant create <name> --config <path>' and delete '${context.tenant.name}'`
          : 'Create the file or pass an existing one',
    };
  }
  let parsed: unknown;
  try {
    parsed = JSON.parse(raw);
  } catch {
    return {
      name,
      status: 'fail',
      detail: `${path} is not valid JSON`,
      hint: 'Fix the file, then re-run doctor',
    };
  }
  if (parsed === null || typeof parsed !== 'object' || Array.isArray(parsed)) {
    return {
      name,
      status: 'fail',
      detail: `${path} is not a JSON object`,
      hint: 'Fix the file — it must contain a single JSON object',
    };
  }
  const result = ConfigSchema.safeParse(parsed);
  if (!result.success) {
    const keys = [...new Set(result.error.issues.map((issue) => issue.path.join('.') || '(root)'))];
    return {
      name,
      status: 'fail',
      detail: `${path} has invalid values for: ${keys.join(', ')}`,
      hint: "Fix them with 'airs tenant set <name> <key>' (values are never printed here)",
    };
  }
  if (context.selection === 'tenant') {
    const fileTsg = (parsed as Record<string, unknown>).mgmtTsgId;
    if (fileTsg !== context.tenant.tsgId) {
      return {
        name,
        status: 'fail',
        detail: `${path} carries a different mgmtTsgId than the registration (TSG ${context.tenant.tsgId})`,
        hint: "Register the file under a new tenant name with 'airs tenant create <name> --config <path>'",
      };
    }
  }
  const known = new Set<string>(Object.keys(ConfigSchema.shape));
  const ignored = Object.keys(parsed).filter((key) => !known.has(key));
  if (ignored.length) {
    const retired = ignored.filter((key) =>
      (RETIRED_CONFIG_KEYS as readonly string[]).includes(key),
    );
    return {
      name,
      status: 'warn',
      detail: `valid at ${path}; ignored ${plural(ignored.length, 'key')}: ${ignored.join(', ')}`,
      hint: retired.length
        ? `Every product authenticates through mgmtTokenEndpoint now (${retired.join(', ')} ignored); remove them with 'airs tenant unset <name> <key>'`
        : "Remove unknown keys with 'airs tenant unset <name> <key>'",
    };
  }
  return {
    name,
    status: 'pass',
    detail:
      context.selection === 'tenant'
        ? `valid tenant config at ${path} (TSG ${context.tenant.tsgId} matches registration)`
        : `valid JSON at ${path}`,
  };
}

/**
 * Check 4: the CLI reads no configuration from the environment. Anything that
 * looks like it should configure the CLI is reported as ignored so stale shell
 * profiles get cleaned up instead of silently doing nothing.
 */
export function checkEnvironment(env: NodeJS.ProcessEnv = process.env): DoctorCheck {
  const name = 'Environment';
  const ignored = ignoredEnvironment(env);
  const diagnostics = SDK_DIAGNOSTIC_ENV_VARS.filter((key) => env[key]);
  const suffix = diagnostics.length ? `; SDK diagnostics on: ${diagnostics.join(', ')}` : '';
  if (ignored.length) {
    return {
      name,
      status: 'warn',
      detail: `ignored ${plural(ignored.length, 'variable')}: ${ignored.join(', ')}${suffix}`,
      hint: "Configuration comes only from tenant files ('airs tenant set <name> <key>'); unset these",
    };
  }
  return { name, status: 'pass', detail: `no configuration variables set${suffix}` };
}

function isSet(entry: ConfigEntry | undefined): boolean {
  const v = entry?.value;
  return v !== undefined && v !== null && String(v) !== '';
}

/** Check 5: scanner credential (airsApiKey or airsApiToken) presence + source. Optional. */
export function checkScannerCredentials(
  inspected: Record<string, ConfigEntry> | undefined,
  context?: ConfigContext,
): DoctorCheck {
  const name = 'Scanner credentials';
  if (!inspected) return notEvaluated(name);
  const set = SCANNER_CREDENTIAL_KEYS.filter((key) => isSet(inspected[key]));
  if (set.length) {
    return {
      name,
      status: 'pass',
      detail: set.map((key) => `${key} (${inspected[key].source})`).join(', '),
    };
  }
  return {
    name,
    status: 'skip',
    detail: 'not configured — runtime scan, bulk-scan and topics eval are unavailable',
    hint: settingRemedy(['airsApiKey'], context),
  };
}

/**
 * Check 6: the shared management credential set. Missing is a failure unless
 * the setup is scanner-only, in which case management commands are simply
 * unavailable (warn).
 */
export function checkManagementCredentials(
  inspected: Record<string, ConfigEntry> | undefined,
  context?: ConfigContext,
  scannerConfigured = false,
): DoctorCheck {
  const name = 'Management credentials';
  if (!inspected) return notEvaluated(name);
  const missing = MANAGEMENT_CREDENTIAL_KEYS.filter((key) => !isSet(inspected[key]));
  if (missing.length === 0) {
    const detail = MANAGEMENT_CREDENTIAL_KEYS.map(
      (key) => `${key} (${inspected[key].source})`,
    ).join(', ');
    return { name, status: 'pass', detail: `set: ${detail}` };
  }
  return {
    name,
    status: scannerConfigured ? 'warn' : 'fail',
    detail: `missing: ${missing.join(', ')} — management, red team, model security, AI Gateway and AgentGuard commands are unavailable`,
    hint: settingRemedy(missing, context),
  };
}

// ---------------------------------------------------------------------------
// Network checks (time-boxed, never throw)
// ---------------------------------------------------------------------------

const TIMED_OUT = Symbol('timed-out');

async function withTimeout<T>(promise: Promise<T>, ms: number): Promise<T | typeof TIMED_OUT> {
  let timer: NodeJS.Timeout | undefined;
  try {
    return await Promise.race([
      promise,
      new Promise<typeof TIMED_OUT>((resolve) => {
        timer = setTimeout(() => resolve(TIMED_OUT), ms);
      }),
    ]);
  } finally {
    clearTimeout(timer);
  }
}

function httpStatus(err: unknown): number | undefined {
  const e = err as { status?: number; statusCode?: number };
  return e?.status ?? e?.statusCode;
}

/**
 * The scanner SDK throws AISecSDKException without an HTTP status — auth
 * rejections surface only in the message (e.g.
 * "AISEC_CLIENT_SIDE_ERROR:Invalid API Key or OAuth Token").
 */
const AUTH_REJECTED_PATTERN =
  /invalid api key|invalid.*oauth token|api key or oauth token|unauthorized|forbidden/i;

/**
 * Check 7: scanner API reachability + key validity.
 *
 * The probe resolves if the endpoint answered 2xx, rejects with an HTTP
 * status otherwise. 401/403 means the key was rejected; any other HTTP
 * status means we got past auth (endpoint reachable, key accepted); no
 * status at all means the network layer failed.
 */
export async function checkScannerApi(
  probe: () => Promise<unknown>,
  hasKey: boolean,
  timeoutMs: number = DOCTOR_TIMEOUT_MS,
): Promise<DoctorCheck> {
  const name = 'Scanner API';
  if (!hasKey) {
    return { name, status: 'skip', detail: 'skipped — no scanner credentials configured' };
  }
  try {
    const result = await withTimeout(probe(), timeoutMs);
    if (result === TIMED_OUT) {
      return {
        name,
        status: 'fail',
        detail: `timed out after ${timeoutMs}ms — network unreachable or endpoint not responding`,
        hint: 'Check network connectivity and the airsApiEndpoint setting',
      };
    }
    return { name, status: 'pass', detail: 'endpoint reachable, API key accepted' };
  } catch (err) {
    const status = httpStatus(err);
    const message = errMessage(err);
    if (status === 401 || status === 403 || AUTH_REJECTED_PATTERN.test(message)) {
      const suffix = status !== undefined ? ` (HTTP ${status})` : '';
      return {
        name,
        status: 'fail',
        detail: `API key rejected${suffix}: ${message}`,
        hint: 'Verify airsApiKey belongs to this tenant and is not expired',
      };
    }
    if (status !== undefined) {
      // Reached the API and got past auth — probe query itself was refused
      // (e.g. unknown scan id), which still proves connectivity + key.
      return {
        name,
        status: 'pass',
        detail: `endpoint reachable, API key accepted (HTTP ${status} on probe query)`,
      };
    }
    return {
      name,
      status: 'fail',
      detail: `network unreachable: ${message}`,
      hint: 'Check network connectivity, proxy settings, and DNS',
    };
  }
}

/** Check 8: management OAuth token fetch via a minimal authenticated call. */
export async function checkManagementAuth(
  probe: () => Promise<number>,
  hasCreds: boolean,
  timeoutMs: number = DOCTOR_TIMEOUT_MS,
): Promise<DoctorCheck> {
  const name = 'Management OAuth';
  if (!hasCreds) {
    return { name, status: 'skip', detail: 'skipped — management credentials not configured' };
  }
  try {
    const result = await withTimeout(probe(), timeoutMs);
    if (result === TIMED_OUT) {
      return {
        name,
        status: 'fail',
        detail: `timed out after ${timeoutMs}ms — network unreachable or endpoint not responding`,
        hint: 'Check network connectivity and the mgmtTokenEndpoint / mgmtEndpoint settings',
      };
    }
    return {
      name,
      status: 'pass',
      detail: `OAuth token obtained, topics API answered (${plural(result, 'custom topic')})`,
    };
  } catch (err) {
    const status = httpStatus(err);
    const message = errMessage(err);
    let detail: string;
    if (status !== undefined) {
      detail = `management API error (HTTP ${status}): ${message}`;
    } else if (AUTH_REJECTED_PATTERN.test(message) || /oauth|invalid_client/i.test(message)) {
      detail = `authentication failed: ${message}`;
    } else {
      detail = `network unreachable: ${message}`;
    }
    return {
      name,
      status: 'fail',
      detail,
      hint: 'Verify mgmtClientId / mgmtClientSecret / mgmtTsgId belong to this tenant',
    };
  }
}

/**
 * Check 9: AI Gateway reachability via the cheapest authenticated data-plane
 * read (workspace list). Shares management credentials, so it is skipped when
 * those are missing. A 403 is a grant problem — surface which grant via
 * {@link aiGatewayGrantHint}.
 */
export async function checkAiGatewayApi(
  probe: () => Promise<number>,
  hasCreds: boolean,
  timeoutMs: number = DOCTOR_TIMEOUT_MS,
): Promise<DoctorCheck> {
  const name = 'AI Gateway API';
  if (!hasCreds) {
    return { name, status: 'skip', detail: 'skipped — management credentials not configured' };
  }
  try {
    const result = await withTimeout(probe(), timeoutMs);
    if (result === TIMED_OUT) {
      return {
        name,
        status: 'fail',
        detail: `timed out after ${timeoutMs}ms — network unreachable or endpoint not responding`,
        hint: 'Check network connectivity to api.apps.paloaltonetworks.com',
      };
    }
    return {
      name,
      status: 'pass',
      detail: `endpoint reachable (${plural(result, 'workspace')} in scope)`,
    };
  } catch (err) {
    const status = httpStatus(err);
    const message = errMessage(err);
    if (status === 403) {
      // Permission boundary, not a broken environment: the endpoint answered
      // and OAuth succeeded — the account just lacks an AI Gateway grant.
      // Warn (exit 0) so preflights don't fail for setups not using the
      // AI Gateway, and say exactly which grant is missing.
      return {
        name,
        status: 'warn',
        detail: `endpoint reachable, but access denied (HTTP 403): ${message}`,
        hint: aiGatewayGrantHint(err),
      };
    }
    return {
      name,
      status: 'fail',
      detail:
        status !== undefined
          ? `AI Gateway API error (HTTP ${status}): ${message}`
          : `network unreachable: ${message}`,
      hint: 'Verify the tenant credentials and network connectivity',
    };
  }
}

// ---------------------------------------------------------------------------
// Orchestration
// ---------------------------------------------------------------------------

export interface DoctorDeps {
  nodeVersion?: string;
  /** Explicit config file; bypasses tenant selection exactly like the library API. */
  configFilePath?: string;
  /** Pre-resolved config source; built from `configFilePath` / the registry when omitted. */
  context?: ConfigContext;
  env?: NodeJS.ProcessEnv;
  inspect?: () => Promise<Record<string, ConfigEntry>>;
  /** Effective config for the default probes; loaded lazily and only when a default probe runs. */
  loadConfig?: () => Promise<Config>;
  /** Cheap scanner-API call; built from config when omitted. */
  scannerProbe?: (config: Config) => Promise<unknown>;
  /** Minimal management call returning a count; built from config when omitted. */
  mgmtProbe?: (config: Config) => Promise<number>;
  /** Minimal AI Gateway call returning a workspace count; built from config when omitted. */
  aiGwProbe?: (config: Config) => Promise<number>;
  timeoutMs?: number;
}

/**
 * Default scanner probe. DESIGN DECISION: the scan API's only surfaces are
 * syncScan/asyncScan (which submit content and burn scan quota) and the
 * results queries. `Scanner.queryByScanIds` is a GET against
 * /v1/scan/results — it authenticates the API key and proves reachability
 * without submitting any content, so it is the cheapest real call. A random
 * UUID is queried; an empty result (or a non-auth HTTP error) still proves
 * the key was accepted.
 */
async function defaultScannerProbe(config: Config): Promise<unknown> {
  init(runtimeInitOptions(config));
  const scanner = new Scanner();
  return scanner.queryByScanIds([randomUUID()]);
}

/** Default management probe: listTopics() — smallest authenticated GET, on the configured endpoints. */
async function defaultMgmtProbe(config: Config): Promise<number> {
  const service = new SdkManagementService(managementClientOptions(config));
  const topics = await service.listTopics();
  return topics.length;
}

/** Default AI Gateway probe: data-plane workspace list — smallest authenticated GET. */
async function defaultAiGwProbe(config: Config): Promise<number> {
  const service = new SdkAiGatewayService(aiGatewayClientOptions(config));
  const workspaces = await service.listWorkspaces();
  return workspaces.length;
}

/** Run all checks in order. Never throws. */
export async function runDoctor(deps: DoctorDeps = {}): Promise<DoctorCheck[]> {
  const env = deps.env ?? process.env;
  const timeoutMs = deps.timeoutMs ?? DOCTOR_TIMEOUT_MS;
  const node = checkNodeVersion(deps.nodeVersion);

  let context = deps.context;
  let tenant: DoctorCheck;
  try {
    context ??= resolveConfigContext(deps.configFilePath);
    tenant = checkTenant(context);
  } catch (err) {
    let registryPath = 'the tenant registry';
    try {
      registryPath = tenantStorePath();
    } catch {
      // XDG_STATE_HOME itself is invalid; the message already says so.
    }
    tenant = tenantRegistryFailure(err, registryPath);
  }
  if (!context) {
    return [node, tenant, ...DOCTOR_CHECK_NAMES.slice(2).map(notEvaluated)];
  }

  const configFile = await checkConfigFile(context);
  const environment = checkEnvironment(env);

  let inspected: Record<string, ConfigEntry> | undefined;
  if (context.selection !== 'none' && configFile.status !== 'fail') {
    try {
      inspected = await (deps.inspect ?? (() => inspectConfig(deps.configFilePath)))();
    } catch {
      // Reported by the config file check.
    }
  }

  const scannerCreds = checkScannerCredentials(inspected, context);
  const hasScanner = scannerCreds.status === 'pass';
  const mgmtCreds = checkManagementCredentials(inspected, context, hasScanner);
  const hasMgmt = mgmtCreds.status === 'pass';

  // Probes read the file the checks above validated, never a second registry lookup.
  const configPath =
    deps.configFilePath ?? (context.selection === 'none' ? undefined : context.path);
  let configPromise: Promise<Config> | undefined;
  const getConfig = () => {
    configPromise ??= (deps.loadConfig ?? (() => loadConfig({}, configPath)))();
    return configPromise;
  };
  const scannerProbe = deps.scannerProbe ?? defaultScannerProbe;
  const mgmtProbe = deps.mgmtProbe ?? defaultMgmtProbe;
  const aiGwProbe = deps.aiGwProbe ?? defaultAiGwProbe;

  const scannerApi = await checkScannerApi(
    async () => scannerProbe(await getConfig()),
    hasScanner,
    timeoutMs,
  );
  const mgmtAuth = await checkManagementAuth(
    async () => mgmtProbe(await getConfig()),
    hasMgmt,
    timeoutMs,
  );
  const aiGwApi = await checkAiGatewayApi(
    async () => aiGwProbe(await getConfig()),
    hasMgmt,
    timeoutMs,
  );

  return [
    node,
    tenant,
    configFile,
    environment,
    scannerCreds,
    mgmtCreds,
    scannerApi,
    mgmtAuth,
    aiGwApi,
  ];
}

/** Exit-code logic: warns and skips are fine, any fail means exit 1. */
export function hasFailure(checks: DoctorCheck[]): boolean {
  return checks.some((c) => c.status === 'fail');
}

// ---------------------------------------------------------------------------
// CLI
// ---------------------------------------------------------------------------

const STATUS_KIND: Record<DoctorStatus, BulletKind> = {
  pass: 'success',
  warn: 'warn',
  fail: 'error',
  skip: 'skip',
};

/** One-line verdict for the pretty report. */
export function summarize(checks: DoctorCheck[]): { failed: boolean; message: string } {
  const count = (status: DoctorStatus) => checks.filter((c) => c.status === status).length;
  const fails = count('fail');
  if (fails > 0) return { failed: true, message: `${plural(fails, 'check')} failed` };
  const notes: string[] = [];
  if (count('warn') > 0) notes.push(plural(count('warn'), 'warning'));
  if (count('skip') > 0) notes.push(`${count('skip')} skipped`);
  return {
    failed: false,
    message: notes.length ? `All checks passed (${notes.join(', ')})` : 'All checks passed',
  };
}

function renderPretty(checks: DoctorCheck[]): void {
  ui.header('Doctor', 'Prisma AIRS CLI preflight checks');
  for (const check of checks) {
    ui.bullet(`${check.name} — ${check.detail}`, STATUS_KIND[check.status]);
    if (check.hint) ui.dim(`    ${check.hint}`);
  }
  console.log('');
  const summary = summarize(checks);
  if (summary.failed) ui.error(summary.message);
  else ui.success(summary.message);
  console.log('');
}

export function registerDoctorCommand(program: Command): void {
  const doctor = program
    .command('doctor')
    .description(
      'Check the selected tenant, its config file, credentials, and API connectivity (preflight)',
    )
    .option('--output <format>', 'Output format: pretty, table, markdown, csv, json, yaml')
    .addHelpText(
      'after',
      examples('airs doctor', `airs doctor --output json | jq '.[] | select(.status != "pass")'`),
    )
    .action(async (opts) => {
      // Doctor must run even when the selected config cannot load (that is
      // what it diagnoses), so output resolution never touches the config.
      const fmt = await resolveOutput(doctor, opts, { ignoreConfig: true });
      const checks = await runDoctor();

      if (fmt === 'pretty') {
        renderPretty(checks);
      } else {
        console.log(
          formatOutput(
            checks.map((check) => ({ ...check })),
            [
              { key: 'name', label: 'Name' },
              { key: 'status', label: 'Status' },
              { key: 'detail', label: 'Detail' },
              { key: 'hint', label: 'Hint' },
            ],
            fmt,
          ),
        );
      }

      // process.exit (not exitCode) — a timed-out probe may hold a pending
      // socket that would otherwise keep the process alive.
      process.exit(hasFailure(checks) ? 1 : 0);
    });
}
