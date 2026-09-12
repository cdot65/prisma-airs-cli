/**
 * The CLI reads no configuration from the environment: tenant config files are the
 * only source. This module exists so `airs doctor` can tell users which variables
 * they still have set that no longer do anything.
 */

/** Read by the SDK on every request; the CLI neither reads nor documents values for them. */
export const SDK_DIAGNOSTIC_ENV_VARS = [
  'PANW_AI_SEC_DEBUG',
  'PANW_AI_SEC_DEBUG_BODY',
  'PANW_AI_SEC_TIMEOUT_MS',
] as const;

/** Where the tenant registry lives; these select files, never configuration values. */
export const REGISTRY_ENV_VARS = ['PRISMA_AIRS_TENANTS_PATH', 'XDG_STATE_HOME'] as const;

/** Names earlier releases read that are now ignored. */
const RETIRED_ENV_VARS = [
  'PRISMA_AIRS_CONFIG_PATH',
  'SCAN_CONCURRENCY',
  'DATA_DIR',
  'MEMORY_ENABLED',
  'MEMORY_DIR',
  'MAX_MEMORY_CHARS',
  'ACCUMULATE_TESTS',
  'MAX_ACCUMULATED_TESTS',
];

function isSet(env: NodeJS.ProcessEnv, name: string): boolean {
  const value = env[name];
  return value !== undefined && value !== '';
}

/**
 * Nonempty variables the CLI ignores: every `PANW_*` name except the SDK diagnostic
 * switches, plus retired names. Sorted; names only, never values.
 */
export function ignoredEnvironment(env: NodeJS.ProcessEnv = process.env): string[] {
  const diagnostics = new Set<string>(SDK_DIAGNOSTIC_ENV_VARS);
  return Object.keys(env)
    .filter(
      (name) =>
        isSet(env, name) &&
        ((name.startsWith('PANW_') && !diagnostics.has(name)) || RETIRED_ENV_VARS.includes(name)),
    )
    .sort();
}
