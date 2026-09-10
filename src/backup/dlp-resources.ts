import {
  AdvancedDataProfileRequestSchema,
  type DataPatternRequest,
  DataPatternRequestSchema,
  type DataPatternResponse,
  DataPatternResponseSchema,
  type DataProfileResponse,
  DataProfileResponseSchema,
  type DictionaryRequest,
  DictionaryRequestSchema,
  type DictionaryResponse,
  DictionaryResponseSchema,
  type ManagementClient,
} from '@cdot65/prisma-airs-sdk';
import { z } from 'zod';

/** Only SDK operations required for portable DLP resource configuration (I-1).
 * No update/patch operations: conflict handling is error|verify|skip until the
 * DLP profile write path is live-verified (P-3).
 */
export interface DlpTransferApi {
  dictionaries: Pick<ManagementClient['dlp']['dictionaries'], 'list' | 'create' | 'get'>;
  patterns: Pick<ManagementClient['dlp']['dataPatterns'], 'list' | 'create' | 'get'>;
  profiles: Pick<ManagementClient['dlp']['dataProfiles'], 'list' | 'create' | 'get'>;
}

export const DLP_RESOURCE_KINDS = ['dictionaries', 'patterns', 'profiles'] as const;
export type DlpResourceKind = (typeof DLP_RESOURCE_KINDS)[number];

export const DlpResourcesBackupSchema = z
  .object({
    version: z.literal(1),
    resourceType: z.literal('dlp-resources'),
    exportedAt: z.string().datetime({ offset: true }),
    source: z.object({ tsgId: z.string().min(1) }).strict(),
    dictionaries: z.array(DictionaryResponseSchema).max(10000),
    patterns: z.array(DataPatternResponseSchema).max(10000),
    profiles: z.array(DataProfileResponseSchema).max(10000),
  })
  .strict();
export type DlpResourcesBackup = z.infer<typeof DlpResourcesBackupSchema>;

type ObjectValue = Record<string, unknown>;
function object(value: unknown): value is ObjectValue {
  return value !== null && typeof value === 'object' && !Array.isArray(value);
}

/** Detection techniques whose custom patterns a JSON backup can recreate (P-2).
 * Everything else is bound to tenant-side resources (EDM datasets, fingerprints,
 * trained models, linked dictionaries) and must be mapped, never created.
 */
const PORTABLE_TECHNIQUES = new Set(['regex', 'weighted_regex', 'file_property']);
/** Lifecycle states excluded when matching destination resources by name. */
const RETIRED_PATTERN = new Set(['deleted', 'disabled']);
const RETIRED_PROFILE = new Set(['deleted', 'disabled']);
/** Response-only metadata keys the server owns; never compared, never reported. */
const SERVER_METADATA_KEYS = new Set([
  'id',
  'version',
  'status',
  'profile_status',
  'tenant_id',
  'license_type',
  'is_parent_managed',
  'audit_metadata',
  'type',
  'advance_data_patterns_rule_request',
  'dictionary_metadata',
  'detection_technique',
  'detection_sub_technique',
  'attributes',
  'keywords',
  'tags',
]);

function pagesLimit(value: number): void {
  if (!Number.isSafeInteger(value) || value < 1 || value > 1000)
    throw new Error('maxPages must be an integer from 1 to 1000');
}

function assertName(name: string): void {
  if (
    !name.trim() ||
    [...name].some((char) => char.charCodeAt(0) < 32 || char.charCodeAt(0) === 127)
  )
    throw new Error('DLP resource names must be nonempty and contain no control characters');
}

interface SpringPage<T> {
  content: T[];
  last?: boolean;
  totalPages?: number;
}

/** Complete Spring-page inventory or explicit failure (I-3). One sweep per stage (§3). */
async function springPages<T>(
  fetch: (page: number) => Promise<SpringPage<T>>,
  maxPages: number,
  label: string,
): Promise<T[]> {
  pagesLimit(maxPages);
  const items: T[] = [];
  for (let page = 0; page < maxPages; page++) {
    const result = await fetch(page);
    items.push(...result.content);
    if (items.length > 10000)
      throw new Error(`Inventory of ${label} exceeds the 10000-record safety limit`);
    const last =
      result.last ??
      (result.totalPages !== undefined
        ? page + 1 >= result.totalPages
        : result.content.length < 100);
    if (last) return items;
    if (!result.content.length) throw new Error(`Inventory of ${label} did not advance`);
  }
  throw new Error(`Inventory of ${label} is incomplete: increase --max-pages`);
}

async function inventories(api: DlpTransferApi, maxPages: number) {
  return {
    dictionaries: await springPages<DictionaryResponse>(
      (page) => api.dictionaries.list({ page, size: 100, keywords: true }),
      maxPages,
      'dictionaries',
    ),
    patterns: await springPages<DataPatternResponse>(
      (page) => api.patterns.list({ page, size: 100 }),
      maxPages,
      'data patterns',
    ),
    profiles: await springPages<DataProfileResponse>(
      (page) => api.profiles.list({ page, size: 100 }),
      maxPages,
      'data profiles',
    ),
  };
}

function canonical(value: unknown): string {
  const order = (v: unknown): unknown =>
    Array.isArray(v)
      ? v.map(order)
      : object(v)
        ? Object.fromEntries(
            Object.entries(v)
              .filter(([, child]) => child !== undefined)
              .sort(([a], [b]) => a.localeCompare(b))
              .map(([k, child]) => [k, order(child)]),
          )
        : v;
  return JSON.stringify(order(value));
}

/** All records carrying a tenant identity must agree; mixed inventories are refused. */
function tenantIdentity(records: Array<{ tenant_id?: string | null }>): string | undefined {
  const values = new Set(
    records.map((record) => record.tenant_id).filter((value): value is string => value != null),
  );
  if (values.size > 1) throw new Error('Inventory reports mixed DLP tenant identities');
  return [...values][0];
}

interface RuleLeaf {
  value: ObjectValue;
  technique: string;
}

/** Walk detection rules; return every expression-tree leaf. Unsupported rule shapes throw. */
function ruleLeaves(profile: DataProfileResponse): RuleLeaf[] {
  const leaves: RuleLeaf[] = [];
  const visit = (node: unknown, depth: number): void => {
    if (depth > 100) throw new Error('Unsupported detection rule: expression tree too deep');
    if (!object(node)) return;
    if (object(node.rule_item)) {
      if (typeof node.rule_item.detection_technique !== 'string')
        throw new Error('Unsupported detection rule: leaf without a detection technique');
      leaves.push({ value: node.rule_item, technique: node.rule_item.detection_technique });
    }
    if (Array.isArray(node.sub_expressions))
      for (const child of node.sub_expressions) visit(child, depth + 1);
  };
  for (const rule of profile.detection_rules ?? []) {
    if (!object(rule)) throw new Error('Unsupported detection rule shape');
    if (rule.rule_type !== 'expression_tree')
      throw new Error(`Unsupported detection rule type: ${String(rule.rule_type)}`);
    visit(rule.expression_tree, 0);
  }
  return leaves;
}

/** Refuse unrecognized tenant-bound references instead of silently transplanting IDs (I-7). */
function assertKnownReferences(body: unknown, known: Set<ObjectValue>, label: string): void {
  const visit = (value: unknown, path: string): void => {
    if (Array.isArray(value)) {
      value.forEach((v, i) => {
        visit(v, `${path}[${i}]`);
      });
      return;
    }
    if (!object(value)) return;
    for (const [key, child] of Object.entries(value)) {
      if (
        child !== null &&
        child !== '' &&
        /(?:^|[-_])(id|uuid)$|(?:Id|ID|Uuid|UUID)$/.test(key) &&
        !(known.has(value) && key === 'id')
      )
        throw new Error(`Unsupported cross-tenant reference in ${label} at ${path}.${key}`);
      visit(child, `${path}.${key}`);
    }
  };
  visit(body, label);
}

function uniqueNames(records: Array<{ name?: string | null }>, label: string): void {
  const names = new Set<string>();
  for (const record of records) {
    const name = record.name;
    if (!name) throw new Error(`Backup contains a ${label} without a name`);
    assertName(name);
    if (names.has(name)) throw new Error(`Backup contains duplicate ${label} names: ${name}`);
    names.add(name);
  }
}

export interface DlpBackupOptions {
  /** Primary resource kinds to export fully; referenced dependencies are always embedded. */
  resources?: DlpResourceKind[];
  maxPages?: number;
  /** Exclude profiles with non-transferable rules instead of failing the export. */
  skipUnsupported?: boolean;
}

export interface DlpBackupResult {
  backup: DlpResourcesBackup;
  /** Profiles excluded under skipUnsupported, with the reason each was excluded. */
  skipped: Array<{ profile: string; reason: string }>;
}

/** Read complete inventories and export selected custom resources with their
 * dependency closure (I-4). Predefined resources referenced by a profile are
 * embedded as resolve-only references, never as creatable content (P-1).
 */
export async function backupDlpResources(
  api: DlpTransferApi,
  tsgId: string,
  options: DlpBackupOptions = {},
): Promise<DlpBackupResult> {
  if (!tsgId) throw new Error('Source TSG ID is required');
  const maxPages = options.maxPages ?? 100;
  const kinds = options.resources ?? [...DLP_RESOURCE_KINDS];
  if (!kinds.length || kinds.some((kind) => !DLP_RESOURCE_KINDS.includes(kind)))
    throw new Error('resources must name dictionaries, patterns and/or profiles');
  const inventory = await inventories(api, maxPages);
  tenantIdentity([...inventory.patterns, ...inventory.profiles]);

  const dictionaries = new Map<string, DictionaryResponse>();
  const patterns = new Map<string, DataPatternResponse>();
  const addDictionary = (record: DictionaryResponse): void => {
    if (!record.id) throw new Error('Inventory dictionary has no id');
    if (record.type !== 'predefined' && !Array.isArray(record.keywords))
      throw new Error(`Dictionary keywords were not returned by the region: ${record.name}`);
    dictionaries.set(record.id, structuredClone(record));
  };
  const addPattern = (record: DataPatternResponse): void => {
    if (!record.id) throw new Error('Inventory data pattern has no id');
    patterns.set(record.id, structuredClone(record));
  };

  if (kinds.includes('dictionaries'))
    for (const record of inventory.dictionaries)
      if (record.type !== 'predefined') addDictionary(record);
  if (kinds.includes('patterns'))
    for (const record of inventory.patterns)
      if (record.type !== 'predefined' && !RETIRED_PATTERN.has(record.status ?? 'active'))
        addPattern(record);

  const profiles: DataProfileResponse[] = [];
  const skipped: DlpBackupResult['skipped'] = [];
  if (kinds.includes('profiles')) {
    const patternById = new Map(inventory.patterns.map((p) => [p.id ?? '', p]));
    const dictionaryById = new Map(inventory.dictionaries.map((d) => [d.id ?? '', d]));
    for (const profile of inventory.profiles) {
      if (profile.type === 'predefined') continue;
      if (RETIRED_PROFILE.has(profile.profile_status ?? 'active')) continue;
      const name = profile.name ?? '(unnamed)';
      try {
        for (const leaf of ruleLeaves(profile)) {
          const id = leaf.value.id;
          if (id == null || id === '') {
            // Inline leaves carry their own definition, except EDM leaves, which
            // reference a tenant-side dataset a backup cannot recreate (P-2).
            if (leaf.value.edm_dataset_id != null)
              throw new Error('references an EDM dataset directly');
            continue;
          }
          if (typeof id !== 'string') throw new Error('has a non-string rule reference id');
          if (leaf.technique === 'dictionary') {
            const record = dictionaryById.get(id);
            if (!record) throw new Error(`references an unknown dictionary: ${id}`);
            // Dictionaries carry no version; a pinned leaf could never rebind (I-4).
            if (leaf.value.version != null)
              throw new Error(`references a version-pinned dictionary: ${id}`);
            addDictionary(record);
          } else {
            const record = patternById.get(id);
            if (!record) throw new Error(`references an unknown data pattern: ${id}`);
            // The exact referenced revision must be the one exported (I-4): the
            // catalog holds only the current version, so a stale pin means the
            // enforced detection no longer matches what a restore would rebuild.
            if (leaf.value.version != null && record.version !== leaf.value.version)
              throw new Error(
                `references data pattern version ${leaf.value.version}, but the catalog holds version ${record.version ?? 'unknown'}: ${id}`,
              );
            addPattern(record);
          }
        }
        profiles.push(structuredClone(profile));
      } catch (error) {
        const reason = error instanceof Error ? error.message : 'unsupported rule';
        if (!options.skipUnsupported)
          throw new Error(`Profile cannot be exported: ${name} ${reason}`);
        skipped.push({ profile: name, reason });
      }
    }
  }

  const byName = (a: { name?: string | null }, b: { name?: string | null }) =>
    (a.name ?? '').localeCompare(b.name ?? '');
  const dictionaryRecords = [...dictionaries.values()].sort(byName);
  const patternRecords = [...patterns.values()].sort(byName);
  profiles.sort(byName);
  uniqueNames(dictionaryRecords, 'dictionary');
  uniqueNames(patternRecords, 'data pattern');
  uniqueNames(profiles, 'data profile');
  if (!dictionaryRecords.length && !patternRecords.length && !profiles.length)
    throw new Error('No matching DLP resources to back up');
  const backup = DlpResourcesBackupSchema.parse({
    version: 1,
    resourceType: 'dlp-resources',
    exportedAt: new Date().toISOString(),
    source: { tsgId },
    dictionaries: dictionaryRecords,
    patterns: patternRecords,
    profiles,
  });
  return { backup, skipped };
}

export interface DlpRestoreOptions {
  namePrefix?: string;
  /** Applies to data profiles. Dependencies always reuse-if-identical or fail. */
  onConflict?: 'error' | 'verify' | 'skip';
  /** Explicit source pattern name → already provisioned destination pattern name (P-2). */
  patternMap?: Record<string, string>;
  maxPages?: number;
}

type DependencyAction = 'create' | 'reuse' | 'resolve' | 'map';
interface DictionaryPlan {
  sourceId: string;
  name: string;
  action: Exclude<DependencyAction, 'map'>;
  metadata?: DictionaryRequest;
  keywords?: string[];
  existing?: DictionaryResponse;
}
interface PatternPlan {
  sourceId: string;
  name: string;
  action: DependencyAction;
  body?: DataPatternRequest;
  existing?: DataPatternResponse;
}
interface ProfilePlan {
  name: string;
  source: DataProfileResponse;
  action: 'create' | 'verify' | 'skip';
  existing?: DataProfileResponse;
}
export interface DlpRestorePlan {
  sourceTsgId: string;
  destinationTsgId: string;
  dictionaries: DictionaryPlan[];
  patterns: PatternPlan[];
  profiles: ProfilePlan[];
  maxPages: number;
}

/** Compare a write request against a read-back or destination record (I-8).
 * Every requested field must echo equal (null and absent unify); fields the
 * server adds inside compared subtrees are collected, reported, never failed.
 */
export function compareEcho(
  request: unknown,
  readBack: unknown,
): { matches: boolean; differences: string[]; serverAdded: string[] } {
  const differences: string[] = [];
  const serverAdded: string[] = [];
  const visit = (source: unknown, target: unknown, path: string, top: boolean): void => {
    const a = source ?? null;
    const b = target ?? null;
    if (a === null && b === null) return;
    if (a === null) {
      serverAdded.push(path);
      return;
    }
    if (b === null) {
      differences.push(path);
      return;
    }
    if (Array.isArray(a) || Array.isArray(b)) {
      if (!Array.isArray(a) || !Array.isArray(b) || a.length !== b.length) {
        differences.push(path);
        return;
      }
      a.forEach((item, index) => {
        visit(item, b[index], `${path}[${index}]`, false);
      });
      return;
    }
    if (object(a) && object(b)) {
      for (const key of new Set([...Object.keys(a), ...Object.keys(b)])) {
        if (top && !(key in a) && SERVER_METADATA_KEYS.has(key)) continue;
        visit(a[key], b[key], `${path}.${key}`, false);
      }
      return;
    }
    if (a !== b) differences.push(path);
  };
  visit(request, readBack, 'body', true);
  return { matches: differences.length === 0, differences, serverAdded };
}

function keywordSet(value: unknown): string[] | undefined {
  if (!Array.isArray(value) || value.some((keyword) => typeof keyword !== 'string'))
    return undefined;
  return [...(value as string[])].sort();
}

/** Dictionary responses echo `original_file_name` under `dictionary_metadata`,
 * not at the top level, so metadata compares as a projection plus that field.
 */
function dictionaryEcho(
  metadata: DictionaryRequest,
  record: DictionaryResponse | undefined,
  keywords: string[],
): { matches: boolean; serverAdded: string[] } {
  const { original_file_name, ...projection } = metadata;
  const echo = compareEcho(projection, record);
  const echoedFile = record?.dictionary_metadata?.original_file_name;
  const echoedKeywords = keywordSet(record?.keywords);
  const matches =
    echo.matches &&
    (echoedFile == null || echoedFile === original_file_name) &&
    echoedKeywords !== undefined &&
    canonical(echoedKeywords) === canonical(keywords);
  return { matches, serverAdded: echo.serverAdded };
}

function dictionaryRequest(source: DictionaryResponse, name: string): DictionaryRequest {
  const metadata = {
    category: source.category,
    name,
    original_file_name: source.dictionary_metadata?.original_file_name ?? 'keywords.txt',
    region_name: source.region_name,
    ...(source.description != null ? { description: source.description } : {}),
    ...(source.is_case_sensitive != null ? { is_case_sensitive: source.is_case_sensitive } : {}),
  };
  const parsed = DictionaryRequestSchema.safeParse(metadata);
  if (!parsed.success)
    throw new Error(`Dictionary cannot be rebuilt from the backup: ${source.name}`);
  return parsed.data;
}

function patternRequest(source: DataPatternResponse, name: string): DataPatternRequest {
  const body = {
    name,
    type: source.type,
    detection_config: source.detection_config,
    ...(source.matching_rules != null ? { matching_rules: source.matching_rules } : {}),
    ...(source.description != null ? { description: source.description } : {}),
    ...(source.tags != null ? { tags: source.tags } : {}),
  };
  const parsed = DataPatternRequestSchema.safeParse(structuredClone(body));
  if (!parsed.success)
    throw new Error(`Data pattern cannot be rebuilt from the backup: ${source.name}`);
  return parsed.data;
}

function portablePattern(record: DataPatternResponse): boolean {
  return (
    PORTABLE_TECHNIQUES.has(record.detection_config?.technique ?? '') &&
    record.detection_config?.technique !== undefined
  );
}

function matchByName<T extends { name?: string | null }>(
  records: T[],
  name: string,
  retired: (record: T) => boolean,
  label: string,
): T | undefined {
  const matches = records.filter((record) => record.name === name && !retired(record));
  if (matches.length > 1) throw new Error(`Ambiguous destination ${label} name: ${name}`);
  return matches[0];
}

/** Validate the entire staged restore and resolve identities before any mutation (I-5). */
export async function planDlpResourcesRestore(
  api: DlpTransferApi,
  input: unknown,
  destinationTsgId: string,
  options: DlpRestoreOptions = {},
): Promise<DlpRestorePlan> {
  const backup = DlpResourcesBackupSchema.parse(input);
  if (!destinationTsgId) throw new Error('Destination TSG ID is required');
  const maxPages = options.maxPages ?? 100;
  pagesLimit(maxPages);
  const prefix = options.namePrefix ?? '';
  if (
    prefix.length > 64 ||
    [...prefix].some((char) => char.charCodeAt(0) < 32 || char.charCodeAt(0) === 127)
  )
    throw new Error('Invalid --name-prefix');
  const conflict = options.onConflict ?? 'error';
  if (!['error', 'verify', 'skip'].includes(conflict)) throw new Error('Invalid conflict policy');
  const patternMap = options.patternMap ?? {};
  uniqueNames(backup.dictionaries, 'dictionary');
  uniqueNames(backup.patterns, 'data pattern');
  uniqueNames(backup.profiles, 'data profile');
  const unknownMapped = Object.keys(patternMap).filter(
    (name) => !backup.patterns.some((p) => p.name === name),
  );
  if (unknownMapped.length)
    throw new Error(`--pattern-map names absent from the backup: ${unknownMapped.join(', ')}`);
  const crossTenant = backup.source.tsgId !== destinationTsgId;

  const destination = await inventories(api, maxPages);
  const sourceTenant = tenantIdentity([...backup.patterns, ...backup.profiles]);
  const destinationTenant = tenantIdentity([...destination.patterns, ...destination.profiles]);
  if (sourceTenant !== undefined && destinationTenant !== undefined) {
    if (crossTenant && sourceTenant === destinationTenant)
      throw new Error(
        'Destination credentials report the source DLP tenant; check the selected tenant before restoring',
      );
    if (!crossTenant && sourceTenant !== destinationTenant)
      throw new Error(
        'Destination DLP tenant does not match the backup source; use distinct tenant credentials deliberately',
      );
  }

  const dictionaryPlans: DictionaryPlan[] = [];
  for (const source of backup.dictionaries) {
    if (!source.id || !source.name) throw new Error('Backup dictionary lacks identity');
    if (source.type === 'predefined') {
      const match = matchByName(destination.dictionaries, source.name, () => false, 'dictionary');
      if (!match?.id || match.type !== 'predefined')
        throw new Error(`Missing predefined destination dictionary: ${source.name}`);
      dictionaryPlans.push({
        sourceId: source.id,
        name: source.name,
        action: 'resolve',
        existing: structuredClone(match),
      });
      continue;
    }
    const name = `${prefix}${source.name}`;
    assertName(name);
    const metadata = dictionaryRequest(source, name);
    const keywords = keywordSet(source.keywords);
    if (!keywords) throw new Error(`Backup dictionary lacks keywords: ${source.name}`);
    const match = matchByName(destination.dictionaries, name, () => false, 'dictionary');
    if (!match) {
      dictionaryPlans.push({ sourceId: source.id, name, action: 'create', metadata, keywords });
      continue;
    }
    if (!match.id) throw new Error(`Destination dictionary has no identity: ${name}`);
    if (!dictionaryEcho(metadata, match, keywords).matches)
      throw new Error(
        `Destination dictionary differs: ${name}; use --name-prefix to avoid changing a shared dictionary`,
      );
    dictionaryPlans.push({
      sourceId: source.id,
      name,
      action: 'reuse',
      metadata,
      keywords,
      existing: structuredClone(match),
    });
  }

  const patternPlans: PatternPlan[] = [];
  for (const source of backup.patterns) {
    if (!source.id || !source.name) throw new Error('Backup data pattern lacks identity');
    const retired = (record: DataPatternResponse) => RETIRED_PATTERN.has(record.status ?? 'active');
    if (source.type === 'predefined') {
      const match = matchByName(destination.patterns, source.name, retired, 'data pattern');
      if (!match?.id || match.type !== 'predefined')
        throw new Error(`Missing predefined destination data pattern: ${source.name}`);
      patternPlans.push({
        sourceId: source.id,
        name: source.name,
        action: 'resolve',
        existing: structuredClone(match),
      });
      continue;
    }
    if (Object.hasOwn(patternMap, source.name)) {
      const target = patternMap[source.name];
      const match = matchByName(destination.patterns, target, retired, 'data pattern');
      if (!match?.id) throw new Error(`Missing mapped destination data pattern: ${target}`);
      patternPlans.push({
        sourceId: source.id,
        name: target,
        action: 'map',
        existing: structuredClone(match),
      });
      continue;
    }
    if (!portablePattern(source))
      throw new Error(
        `Data pattern uses a tenant-bound detection technique: ${source.name}; provision it in the destination and bind it with --pattern-map "${source.name}=<destination-name>"`,
      );
    const name = `${prefix}${source.name}`;
    assertName(name);
    const body = patternRequest(source, name);
    if (crossTenant) assertKnownReferences(body, new Set(), `pattern ${source.name}`);
    const match = matchByName(destination.patterns, name, retired, 'data pattern');
    if (!match) {
      patternPlans.push({ sourceId: source.id, name, action: 'create', body });
      continue;
    }
    if (!match.id) throw new Error(`Destination data pattern has no identity: ${name}`);
    if (!compareEcho(body, match).matches)
      throw new Error(
        `Destination data pattern differs: ${name}; use --name-prefix to avoid changing a shared pattern`,
      );
    patternPlans.push({
      sourceId: source.id,
      name,
      action: 'reuse',
      body,
      existing: structuredClone(match),
    });
  }

  const profilePlans: ProfilePlan[] = [];
  for (const source of backup.profiles) {
    if (!source.name) throw new Error('Backup data profile lacks identity');
    if (source.type === 'predefined') throw new Error('Backup contains a predefined data profile');
    const name = `${prefix}${source.name}`;
    assertName(name);
    const match = matchByName(
      destination.profiles,
      name,
      (record) => RETIRED_PROFILE.has(record.profile_status ?? 'active'),
      'data profile',
    );
    if (match && conflict === 'error')
      throw new Error(
        `Destination data profile already exists: ${name}; choose --on-conflict verify or skip`,
      );
    if (match && !match.id) throw new Error(`Destination data profile has no identity: ${name}`);
    profilePlans.push({
      name,
      source: structuredClone(source),
      existing: match ? structuredClone(match) : undefined,
      action: match ? (conflict === 'skip' ? 'skip' : 'verify') : 'create',
    });
  }

  const plan: DlpRestorePlan = {
    sourceTsgId: backup.source.tsgId,
    destinationTsgId,
    dictionaries: dictionaryPlans,
    patterns: patternPlans,
    profiles: profilePlans,
    maxPages,
  };

  // Validate every changing profile as the request it will become (I-5), and
  // verify resumed profiles before ANY dependency writes (I-9), which requires
  // all of their dependencies to already exist in the destination.
  const bindings = planBindings(plan);
  const planned = new Set([
    ...dictionaryPlans.map((d) => d.sourceId),
    ...patternPlans.map((p) => p.sourceId),
  ]);
  for (const item of profilePlans) {
    if (item.action === 'skip') continue;
    for (const leaf of ruleLeaves(item.source)) {
      const id = leaf.value.id;
      if (id != null && id !== '' && !planned.has(String(id)))
        throw new Error(
          `Backup data profile references a resource absent from the backup: ${item.name}`,
        );
    }
    const body = profileRequest(item, bindings, crossTenant, item.action === 'verify');
    if (item.action === 'verify' && body && !compareEcho(body, item.existing).matches)
      throw new Error(
        `Destination data profile does not match restore plan: ${item.name}; verification made no changes`,
      );
  }
  return plan;
}

interface Binding {
  id: string;
  name: string;
  version?: number;
}

/** Bindings resolvable at plan time: everything except records still to create. */
function planBindings(plan: DlpRestorePlan): Map<string, Binding> {
  const bindings = new Map<string, Binding>();
  for (const item of plan.dictionaries)
    if (item.existing?.id) bindings.set(item.sourceId, { id: item.existing.id, name: item.name });
  for (const item of plan.patterns)
    if (item.existing?.id)
      bindings.set(item.sourceId, {
        id: item.existing.id,
        name: item.name,
        version: item.existing.version ?? undefined,
      });
  return bindings;
}

/** Build the remapped create request for a profile plan. When `requireBindings`
 * is false, a reference to a still-to-create dependency defers to execute time
 * by returning undefined; verification paths always require complete bindings.
 */
function profileRequest(
  item: ProfilePlan,
  bindings: Map<string, Binding>,
  crossTenant: boolean,
  requireBindings: boolean,
): ReturnType<typeof AdvancedDataProfileRequestSchema.parse> | undefined {
  const detectionRules = structuredClone(item.source.detection_rules ?? []);
  const known = new Set<ObjectValue>();
  let deferred = false;
  const remapped = { ...structuredClone(item.source), detection_rules: detectionRules };
  for (const leaf of ruleLeaves(remapped as DataProfileResponse)) {
    known.add(leaf.value);
    const id = leaf.value.id;
    if (id == null || id === '') continue;
    const binding = bindings.get(String(id));
    if (!binding) {
      if (requireBindings)
        throw new Error(
          `Missing planned dependency binding; an existing data profile cannot be verified without its destination dependencies: ${item.name}`,
        );
      deferred = true;
      continue;
    }
    leaf.value.id = binding.id;
    if (leaf.value.name != null) leaf.value.name = binding.name;
    if (leaf.value.version != null) {
      if (binding.version === undefined)
        throw new Error(`Destination data pattern version is unknown: ${binding.name}`);
      leaf.value.version = binding.version;
    }
  }
  const body = {
    name: item.name,
    detection_rules: detectionRules,
    ...(item.source.profile_type != null ? { profile_type: item.source.profile_type } : {}),
    ...(item.source.description != null ? { description: item.source.description } : {}),
    ...(item.source.is_granular_data_profile != null
      ? { is_granular_data_profile: item.source.is_granular_data_profile }
      : {}),
  };
  // Scan the pre-parse body: `known` holds leaf references by identity, which a
  // Zod parse would not preserve. Leaf ids are understood references (source ids
  // awaiting remap, or already remapped destination ids); everything else fails.
  if (crossTenant) assertKnownReferences(body, known, `profile ${item.name}`);
  const parsed = AdvancedDataProfileRequestSchema.parse(body);
  return deferred ? undefined : parsed;
}

export interface DlpRestoreResult {
  sourceTsgId: string;
  destinationTsgId: string;
  complete: boolean;
  dictionaries: Array<{ name: string; action: 'created' | 'reused' | 'resolved'; id: string }>;
  patterns: Array<{
    name: string;
    action: 'created' | 'reused' | 'resolved' | 'mapped';
    id: string;
  }>;
  profiles: Array<{ name: string; action: 'created' | 'verified' | 'skipped'; id?: string }>;
  serverAdded: Array<{ resource: string; fields: string[] }>;
  error?: string;
}

const DEPENDENCY_ACTIONS = {
  create: 'created',
  reuse: 'reused',
  resolve: 'resolved',
  map: 'mapped',
} as const;

function assertUnchanged(current: unknown, prior: unknown, label: string): void {
  if (canonical(current ?? null) !== canonical(prior ?? null))
    throw new Error(`Destination ${label} changed after planning`);
}

/** Execute a bound plan stage by stage; stop on the first failure, report every
 * completed write, and never roll back silently (I-9).
 */
export async function restoreDlpResources(
  api: DlpTransferApi,
  plan: DlpRestorePlan,
): Promise<DlpRestoreResult> {
  const result: DlpRestoreResult = {
    sourceTsgId: plan.sourceTsgId,
    destinationTsgId: plan.destinationTsgId,
    complete: false,
    dictionaries: [],
    patterns: [],
    profiles: [],
    serverAdded: [],
  };
  const crossTenant = plan.sourceTsgId !== plan.destinationTsgId;
  try {
    // Recheck the full plan after confirmation, before the first write (I-6).
    const current = await inventories(api, plan.maxPages);
    for (const item of plan.dictionaries) {
      const match = matchByName(current.dictionaries, item.name, () => false, 'dictionary');
      if (item.existing) assertUnchanged(match, item.existing, `dictionary: ${item.name}`);
      else if (match)
        throw new Error(`Destination dictionary changed after planning: ${item.name}`);
    }
    const retiredPattern = (record: DataPatternResponse) =>
      RETIRED_PATTERN.has(record.status ?? 'active');
    for (const item of plan.patterns) {
      const match = matchByName(current.patterns, item.name, retiredPattern, 'data pattern');
      if (item.existing) assertUnchanged(match, item.existing, `data pattern: ${item.name}`);
      else if (match)
        throw new Error(`Destination data pattern changed after planning: ${item.name}`);
    }
    for (const item of plan.profiles) {
      const match = matchByName(
        current.profiles,
        item.name,
        (record) => RETIRED_PROFILE.has(record.profile_status ?? 'active'),
        'data profile',
      );
      if (item.existing) assertUnchanged(match, item.existing, `data profile: ${item.name}`);
      else if (match)
        throw new Error(`Destination data profile changed after planning: ${item.name}`);
    }

    const bindings = planBindings(plan);
    for (const item of plan.dictionaries) {
      if (item.action !== 'create') {
        const id = item.existing?.id;
        if (!id) throw new Error(`Destination dictionary has no identity: ${item.name}`);
        result.dictionaries.push({ name: item.name, action: DEPENDENCY_ACTIONS[item.action], id });
        continue;
      }
      if (!item.metadata || !item.keywords) throw new Error('Dictionary plan is incomplete');
      const created = await api.dictionaries.create({
        metadata: item.metadata,
        file: `${item.keywords.join('\n')}\n`,
        includeKeywords: true,
      });
      if (!created.id) throw new Error(`Restored dictionary did not verify: ${item.name}`);
      const readBack = await api.dictionaries.get(created.id, { includeKeywords: true });
      const echo = dictionaryEcho(item.metadata, readBack, item.keywords);
      if (!echo.matches || readBack.name !== item.name)
        throw new Error(`Restored dictionary did not verify: ${item.name}`);
      if (echo.serverAdded.length)
        result.serverAdded.push({ resource: `dictionary ${item.name}`, fields: echo.serverAdded });
      bindings.set(item.sourceId, { id: created.id, name: item.name });
      result.dictionaries.push({ name: item.name, action: 'created', id: created.id });
    }

    for (const item of plan.patterns) {
      if (item.action !== 'create') {
        const id = item.existing?.id;
        if (!id) throw new Error(`Destination data pattern has no identity: ${item.name}`);
        result.patterns.push({ name: item.name, action: DEPENDENCY_ACTIONS[item.action], id });
        continue;
      }
      if (!item.body) throw new Error('Data pattern plan is incomplete');
      const created = await api.patterns.create(item.body);
      if (!created.id) throw new Error(`Restored data pattern did not verify: ${item.name}`);
      const readBack = await api.patterns.get(created.id);
      const echo = compareEcho(item.body, readBack);
      // Lifecycle state is server-owned but load-bearing: a retired record is
      // invisible to name matching, so a "created" result must read back live.
      if (
        !echo.matches ||
        readBack.name !== item.name ||
        RETIRED_PATTERN.has(readBack.status ?? 'active')
      )
        throw new Error(`Restored data pattern did not verify: ${item.name}`);
      if (echo.serverAdded.length)
        result.serverAdded.push({
          resource: `data pattern ${item.name}`,
          fields: echo.serverAdded,
        });
      bindings.set(item.sourceId, {
        id: created.id,
        name: item.name,
        version: readBack.version ?? undefined,
      });
      result.patterns.push({ name: item.name, action: 'created', id: created.id });
    }

    for (const item of plan.profiles) {
      if (item.action === 'skip') {
        result.profiles.push({
          name: item.name,
          action: 'skipped',
          id: item.existing?.id ?? undefined,
        });
        continue;
      }
      const body = profileRequest(item, bindings, crossTenant, true);
      if (!body) throw new Error(`Missing planned dependency binding: ${item.name}`);
      if (item.action === 'verify') {
        const echo = compareEcho(body, item.existing);
        if (!echo.matches)
          throw new Error(`Destination data profile does not match restore plan: ${item.name}`);
        if (echo.serverAdded.length)
          result.serverAdded.push({
            resource: `data profile ${item.name}`,
            fields: echo.serverAdded,
          });
        result.profiles.push({
          name: item.name,
          action: 'verified',
          id: item.existing?.id ?? undefined,
        });
        continue;
      }
      const created = await api.profiles.create(body);
      if (!created.id) throw new Error(`Restored data profile did not verify: ${item.name}`);
      const readBack = await api.profiles.get(created.id);
      const echo = compareEcho(body, readBack);
      if (
        !echo.matches ||
        readBack.name !== item.name ||
        RETIRED_PROFILE.has(readBack.profile_status ?? 'active')
      )
        throw new Error(`Restored data profile did not verify: ${item.name}`);
      if (echo.serverAdded.length)
        result.serverAdded.push({
          resource: `data profile ${item.name}`,
          fields: echo.serverAdded,
        });
      result.profiles.push({ name: item.name, action: 'created', id: created.id });
    }
    result.complete = true;
  } catch (error) {
    // SDK errors can include request/response content; publish only safe local messages or status.
    const status = (error as { statusCode?: number })?.statusCode;
    result.error = Number.isInteger(status)
      ? `API request failed (HTTP ${status}); verify destination state before retrying`
      : error instanceof Error &&
          /^(Destination|Restored|Missing|Inventory|Ambiguous|Dictionary|Data pattern|Data profile)/.test(
            error.message,
          )
        ? error.message
        : 'Restore stopped; verify destination state before retrying';
  }
  return result;
}
