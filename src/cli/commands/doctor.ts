import { randomUUID } from 'node:crypto';
import { readFile } from 'node:fs/promises';
import { init, Scanner } from '@cdot65/prisma-airs-sdk';
import type { Command } from 'commander';
import { SdkManagementService } from '../../airs/management.js';
import { runtimeInitOptions } from '../../config/client-options.js';
import {
  type ConfigEntry,
  inspectConfig,
  loadConfig,
  resolveConfigFilePath,
} from '../../config/loader.js';
import { examples } from '../examples.js';
import { type BulletKind, ui, usageError } from '../renderer/index.js';

export type DoctorStatus = 'pass' | 'warn' | 'fail';

export interface DoctorCheck {
  name: string;
  status: DoctorStatus;
  detail: string;
  hint?: string;
}

/** Time box for each network check. */
export const DOCTOR_TIMEOUT_MS = 5000;

const MIN_NODE_MAJOR = 20;

// ---------------------------------------------------------------------------
// Pure checks
// ---------------------------------------------------------------------------

/** Check 1: Node.js runtime version >= 20. */
export function checkNodeVersion(version: string = process.version): DoctorCheck {
  const major = Number.parseInt(version.replace(/^v/, ''), 10);
  if (Number.isFinite(major) && major >= MIN_NODE_MAJOR) {
    return { name: 'Node.js version', status: 'pass', detail: `${version} (>= 20 required)` };
  }
  return {
    name: 'Node.js version',
    status: 'fail',
    detail: `${version} is below the required Node ${MIN_NODE_MAJOR}`,
    hint: `Upgrade to Node ${MIN_NODE_MAJOR}+ (e.g. via nvm or your package manager)`,
  };
}

/** Check 2: config file — absent is fine (env-only setups), malformed is not. */
export async function checkConfigFile(filePath: string): Promise<DoctorCheck> {
  const name = 'Config file';
  let raw: string;
  try {
    raw = await readFile(filePath, 'utf-8');
  } catch {
    return {
      name,
      status: 'warn',
      detail: `not found at ${filePath} — using env vars and defaults`,
      hint: "Create one with 'airs config set <key> <value>' (optional)",
    };
  }
  try {
    const parsed = JSON.parse(raw);
    if (parsed === null || typeof parsed !== 'object' || Array.isArray(parsed)) {
      return {
        name,
        status: 'fail',
        detail: `${filePath} is not a JSON object`,
        hint: 'Fix or delete the file — it must contain a single JSON object',
      };
    }
    return { name, status: 'pass', detail: `valid JSON at ${filePath}` };
  } catch {
    return {
      name,
      status: 'fail',
      detail: `${filePath} is not valid JSON`,
      hint: 'Fix or delete the file, then re-run doctor',
    };
  }
}

function isSet(entry: ConfigEntry | undefined): boolean {
  const v = entry?.value;
  return v !== undefined && v !== null && String(v) !== '';
}

/** Check 3a: scanner credential (airsApiKey) presence + source. */
export function checkScannerCredentials(inspected: Record<string, ConfigEntry>): DoctorCheck {
  const name = 'Scanner credentials';
  const key = inspected.airsApiKey;
  if (isSet(key)) {
    return { name, status: 'pass', detail: `airsApiKey set (${key.source})` };
  }
  return {
    name,
    status: 'fail',
    detail: 'airsApiKey is not set',
    hint: "Set PANW_AI_SEC_API_KEY or run 'airs config set airsApiKey <key>'",
  };
}

const MGMT_KEYS: Array<{ key: string; envVar: string }> = [
  { key: 'mgmtClientId', envVar: 'PANW_MGMT_CLIENT_ID' },
  { key: 'mgmtClientSecret', envVar: 'PANW_MGMT_CLIENT_SECRET' },
  { key: 'mgmtTsgId', envVar: 'PANW_MGMT_TSG_ID' },
];

/** Check 3b: management credential group (clientId + clientSecret + tsgId). */
export function checkManagementCredentials(inspected: Record<string, ConfigEntry>): DoctorCheck {
  const name = 'Management credentials';
  const missing = MGMT_KEYS.filter(({ key }) => !isSet(inspected[key]));
  if (missing.length === 0) {
    const detail = MGMT_KEYS.map(({ key }) => `${key} (${inspected[key].source})`).join(', ');
    return { name, status: 'pass', detail: `set: ${detail}` };
  }
  return {
    name,
    status: 'fail',
    detail: `missing: ${missing.map((m) => m.key).join(', ')}`,
    hint: `Set ${missing.map((m) => m.envVar).join(', ')} (or 'airs config set …')`,
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

function errMessage(err: unknown): string {
  return err instanceof Error ? err.message : String(err);
}

/**
 * The scanner SDK throws AISecSDKException without an HTTP status — auth
 * rejections surface only in the message (e.g.
 * "AISEC_CLIENT_SIDE_ERROR:Invalid API Key or OAuth Token").
 */
const AUTH_REJECTED_PATTERN =
  /invalid api key|invalid.*oauth token|api key or oauth token|unauthorized|forbidden/i;

/**
 * Check 4: scanner API reachability + key validity.
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
    return {
      name,
      status: 'warn',
      detail: 'skipped — no scanner API key configured',
      hint: 'Set PANW_AI_SEC_API_KEY to enable this check',
    };
  }
  try {
    const result = await withTimeout(probe(), timeoutMs);
    if (result === TIMED_OUT) {
      return {
        name,
        status: 'fail',
        detail: `timed out after ${timeoutMs}ms — network unreachable or endpoint not responding`,
        hint: 'Check network connectivity and PANW_AI_SEC_API_ENDPOINT',
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
        hint: 'Verify PANW_AI_SEC_API_KEY belongs to this tenant and is not expired',
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
      detail: `network unreachable: ${errMessage(err)}`,
      hint: 'Check network connectivity, proxy settings, and DNS',
    };
  }
}

/** Check 5: management OAuth token fetch via a minimal authenticated call. */
export async function checkManagementAuth(
  probe: () => Promise<number>,
  hasCreds: boolean,
  timeoutMs: number = DOCTOR_TIMEOUT_MS,
): Promise<DoctorCheck> {
  const name = 'Management OAuth';
  if (!hasCreds) {
    return {
      name,
      status: 'warn',
      detail: 'skipped — management credentials not configured',
      hint: 'Set PANW_MGMT_CLIENT_ID, PANW_MGMT_CLIENT_SECRET, PANW_MGMT_TSG_ID',
    };
  }
  try {
    const result = await withTimeout(probe(), timeoutMs);
    if (result === TIMED_OUT) {
      return {
        name,
        status: 'fail',
        detail: `timed out after ${timeoutMs}ms — network unreachable or endpoint not responding`,
        hint: 'Check network connectivity and PANW_MGMT_TOKEN_ENDPOINT',
      };
    }
    return {
      name,
      status: 'pass',
      detail: `OAuth token obtained, topics API answered (${result} custom topic${result === 1 ? '' : 's'})`,
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
      hint: 'Verify PANW_MGMT_CLIENT_ID / PANW_MGMT_CLIENT_SECRET / PANW_MGMT_TSG_ID',
    };
  }
}

// ---------------------------------------------------------------------------
// Orchestration
// ---------------------------------------------------------------------------

export interface DoctorDeps {
  nodeVersion?: string;
  configFilePath?: string;
  inspect?: () => Promise<Record<string, ConfigEntry>>;
  /** Cheap scanner-API call; built from config when omitted. */
  scannerProbe?: () => Promise<unknown>;
  /** Minimal management call returning a count; built from config when omitted. */
  mgmtProbe?: () => Promise<number>;
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
async function defaultScannerProbe(): Promise<unknown> {
  const config = await loadConfig();
  init(runtimeInitOptions(config));
  const scanner = new Scanner();
  return scanner.queryByScanIds([randomUUID()]);
}

/** Default management probe: listTopics() — smallest authenticated GET. */
async function defaultMgmtProbe(): Promise<number> {
  const config = await loadConfig();
  const service = new SdkManagementService({
    clientId: config.mgmtClientId,
    clientSecret: config.mgmtClientSecret,
    tsgId: config.mgmtTsgId,
    tokenEndpoint: config.mgmtTokenEndpoint,
  });
  const topics = await service.listTopics();
  return topics.length;
}

/** Run all checks in order. Never throws. */
export async function runDoctor(deps: DoctorDeps = {}): Promise<DoctorCheck[]> {
  const configFilePath = deps.configFilePath ?? resolveConfigFilePath();
  const inspect = deps.inspect ?? (() => inspectConfig(configFilePath));
  const timeoutMs = deps.timeoutMs ?? DOCTOR_TIMEOUT_MS;

  const node = checkNodeVersion(deps.nodeVersion);
  const configFile = await checkConfigFile(configFilePath);

  let inspectedConfig: Record<string, ConfigEntry> = {};
  try {
    inspectedConfig = await inspect();
  } catch {
    // Malformed config already reported by the file check; creds checks
    // proceed against an empty view.
  }

  const scannerCreds = checkScannerCredentials(inspectedConfig);
  const mgmtCreds = checkManagementCredentials(inspectedConfig);

  const scannerApi = await checkScannerApi(
    deps.scannerProbe ?? defaultScannerProbe,
    scannerCreds.status === 'pass',
    timeoutMs,
  );
  const mgmtAuth = await checkManagementAuth(
    deps.mgmtProbe ?? defaultMgmtProbe,
    mgmtCreds.status === 'pass',
    timeoutMs,
  );

  return [node, configFile, scannerCreds, mgmtCreds, scannerApi, mgmtAuth];
}

/** Exit-code logic: warns are fine, any fail means exit 1. */
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
};

const DOCTOR_FORMATS = ['pretty', 'json', 'yaml'] as const;
type DoctorFormat = (typeof DOCTOR_FORMATS)[number];

function parseDoctorFormat(value: string): DoctorFormat {
  if (!(DOCTOR_FORMATS as readonly string[]).includes(value)) {
    usageError(`Invalid --output '${value}'. Valid formats: ${DOCTOR_FORMATS.join(', ')}`);
  }
  return value as DoctorFormat;
}

function toYaml(checks: DoctorCheck[]): string {
  return checks
    .map((c) => {
      const lines = [`name: ${c.name}`, `status: ${c.status}`, `detail: ${c.detail}`];
      if (c.hint) lines.push(`hint: ${c.hint}`);
      return lines.join('\n');
    })
    .join('\n---\n');
}

function renderPretty(checks: DoctorCheck[]): void {
  ui.header('Doctor', 'Prisma AIRS CLI preflight checks');
  for (const check of checks) {
    ui.bullet(`${check.name} — ${check.detail}`, STATUS_KIND[check.status]);
    if (check.hint) ui.dim(`    ${check.hint}`);
  }
  console.log('');
  const fails = checks.filter((c) => c.status === 'fail').length;
  const warns = checks.filter((c) => c.status === 'warn').length;
  if (fails > 0) {
    ui.error(`${fails} check${fails === 1 ? '' : 's'} failed`);
  } else if (warns > 0) {
    ui.success(`All checks passed (${warns} warning${warns === 1 ? '' : 's'})`);
  } else {
    ui.success('All checks passed');
  }
  console.log('');
}

export function registerDoctorCommand(program: Command): void {
  program
    .command('doctor')
    .description('Check credentials, config, and API connectivity (preflight)')
    .option('--output <format>', 'Output format: pretty, json, or yaml', 'pretty')
    .addHelpText(
      'after',
      examples('airs doctor', `airs doctor --output json | jq '.[] | select(.status != "pass")'`),
    )
    .action(async (opts) => {
      const fmt = parseDoctorFormat(opts.output);
      const checks = await runDoctor();

      if (fmt === 'json') {
        console.log(JSON.stringify(checks, null, 2));
      } else if (fmt === 'yaml') {
        console.log(toYaml(checks));
      } else {
        renderPretty(checks);
      }

      // process.exit (not exitCode) — a timed-out probe may hold a pending
      // socket that would otherwise keep the process alive.
      process.exit(hasFailure(checks) ? 1 : 0);
    });
}
