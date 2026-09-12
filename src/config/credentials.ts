import { type ConfigContext, resolveConfigContext } from './loader.js';
import type { Config } from './schema.js';

/** Shared SCM OAuth credential set used by every management-plane product. */
export const MANAGEMENT_CREDENTIAL_KEYS = [
  'mgmtClientId',
  'mgmtClientSecret',
  'mgmtTsgId',
] as const;

/** Either scanner credential satisfies the runtime scan API. */
export const SCANNER_CREDENTIAL_KEYS = ['airsApiKey', 'airsApiToken'] as const;

function present(value: unknown): boolean {
  return typeof value === 'string' && value.trim() !== '';
}

/** Management credential keys that are unset or blank. */
export function missingManagementCredentials(config: Partial<Config>): string[] {
  return MANAGEMENT_CREDENTIAL_KEYS.filter((key) => !present(config[key]));
}

export function hasScannerCredentials(config: Partial<Config>): boolean {
  return SCANNER_CREDENTIAL_KEYS.some((key) => present(config[key]));
}

function safeContext(): ConfigContext | undefined {
  try {
    return resolveConfigContext();
  } catch {
    return undefined;
  }
}

/** Tell the user where a setting belongs for the config source in force. */
export function settingRemedy(
  keys: readonly (keyof Config)[],
  context: ConfigContext | undefined = safeContext(),
): string {
  const list = keys.join(', ');
  if (context?.selection === 'tenant') {
    return `Run 'airs tenant set ${context.tenant.name} <key>' for ${list} (secrets prompt hidden, or use --stdin)`;
  }
  if (context?.selection === 'explicit') return `Add ${list} to ${context.path}`;
  return `Run 'airs tenant create <name>' (prompts for the OAuth credentials), then 'airs tenant switch <name>'`;
}

/** Fail before any SDK client is built so the SDK never resolves credentials on its own. */
export function assertManagementCredentials(
  config: Partial<Config>,
  context?: ConfigContext,
): void {
  const missing = missingManagementCredentials(config);
  if (missing.length === 0) return;
  throw new Error(
    `Management credentials are not configured (missing ${missing.join(', ')}). ${settingRemedy(missing as (keyof Config)[], context)}`,
  );
}

export function assertScannerCredentials(config: Partial<Config>, context?: ConfigContext): void {
  if (hasScannerCredentials(config)) return;
  throw new Error(
    `Scanner credentials are not configured (airsApiKey or airsApiToken). ${settingRemedy(['airsApiKey'], context)}`,
  );
}
