import {
  CreateSecurityProfileRequestSchema,
  CustomTopicSchema,
  type DataProfileResponse,
  type ManagementClient,
  PolicySchema,
  type SecurityProfile,
  SecurityProfileSchema,
} from '@cdot65/prisma-airs-sdk';
import { z } from 'zod';

/** Only SDK operations required for portable Runtime profile configuration. */
export interface ProfileTransferApi {
  profiles: Pick<ManagementClient['profiles'], 'list' | 'create' | 'update'>;
  topics: Pick<ManagementClient['topics'], 'list' | 'create'>;
  dataProfiles: Pick<ManagementClient['dlp']['dataProfiles'], 'list'>;
}

export const RuntimeProfilesBackupSchema = z
  .object({
    version: z.literal(1),
    resourceType: z.literal('runtime-security-profiles'),
    exportedAt: z.string().datetime({ offset: true }),
    source: z.object({ tsgId: z.string().min(1) }).strict(),
    profiles: z.array(SecurityProfileSchema).min(1).max(10000),
    topics: z.array(CustomTopicSchema).max(10000),
  })
  .strict();
export type RuntimeProfilesBackup = z.infer<typeof RuntimeProfilesBackupSchema>;
type Topic = z.infer<typeof CustomTopicSchema>;
type ObjectValue = Record<string, unknown>;
type TopicReference = { topic_id: string; topic_name: string; revision: number } & ObjectValue;

function object(value: unknown): value is ObjectValue {
  return value !== null && typeof value === 'object' && !Array.isArray(value);
}

function pagesLimit(value: number): void {
  if (!Number.isSafeInteger(value) || value < 1 || value > 1000)
    throw new Error('maxPages must be an integer from 1 to 1000');
}

async function offsetPages<T>(
  fetch: (offset: number) => Promise<{ items: T[]; next?: number }>,
  maxPages: number,
): Promise<T[]> {
  pagesLimit(maxPages);
  const items: T[] = [];
  let offset = 0;
  for (let page = 0; page < maxPages; page++) {
    const result = await fetch(offset);
    items.push(...result.items);
    if (items.length > 10000) throw new Error('Inventory exceeds the 10000-record safety limit');
    if (result.next === undefined || result.next === 0) return items;
    if (!result.items.length || !Number.isSafeInteger(result.next) || result.next <= offset)
      throw new Error('Inventory pagination did not advance');
    offset = result.next;
  }
  throw new Error('Inventory is incomplete: increase --max-pages before exporting or restoring');
}

/** Select the latest profile revision without conflating unrelated identities. */
function latestProfiles(profiles: SecurityProfile[]): SecurityProfile[] {
  const names = new Map<string, SecurityProfile>();
  const revisions = new Set<string>();
  for (const profile of profiles) {
    assertName(profile.profile_name);
    const identity = JSON.stringify([profile.profile_name, profile.profile_id, profile.revision]);
    if (revisions.has(identity)) throw new Error('Inventory contains repeated profile records');
    revisions.add(identity);
    const prior = names.get(profile.profile_name);
    if (prior && prior.profile_id !== profile.profile_id)
      throw new Error(`Ambiguous profile name: ${profile.profile_name}`);
    if (!prior || (profile.revision ?? 0) > (prior.revision ?? 0))
      names.set(profile.profile_name, profile);
  }
  return [...names.values()].sort((a, b) => a.profile_name.localeCompare(b.profile_name));
}

function assertName(name: string): void {
  if (
    !name.trim() ||
    [...name].some((char) => char.charCodeAt(0) < 32 || char.charCodeAt(0) === 127)
  )
    throw new Error('Profile/topic names must be nonempty and contain no control characters');
}

function assertTenant(profiles: SecurityProfile[], tsgId: string): void {
  if (profiles.some((profile) => profile.tsg_id != null && String(profile.tsg_id) !== tsgId))
    throw new Error('Destination/source profile tenant identity does not match the selected TSG');
}

async function profiles(api: ProfileTransferApi, maxPages: number) {
  return latestProfiles(
    await offsetPages(async (offset) => {
      const page = await api.profiles.list({ offset, limit: 100, latest: true });
      return { items: page.ai_profiles, next: page.next_offset };
    }, maxPages),
  );
}

async function topics(api: ProfileTransferApi, maxPages: number) {
  return offsetPages(async (offset) => {
    const page = await api.topics.list({ offset, limit: 100 });
    return { items: page.custom_topics, next: page.next_offset };
  }, maxPages);
}

function topicReferences(policy: unknown): TopicReference[] {
  const refs: TopicReference[] = [];
  const visit = (value: unknown) => {
    if (Array.isArray(value)) {
      value.forEach(visit);
      return;
    }
    if (!object(value)) return;
    if ('topic_id' in value) {
      if (
        typeof value.topic_id !== 'string' ||
        typeof value.topic_name !== 'string' ||
        !Number.isSafeInteger(value.revision) ||
        Number(value.revision) < 0
      )
        throw new Error('Invalid topic identity in profile policy');
      refs.push(value as TopicReference);
    }
    Object.values(value).forEach(visit);
  };
  visit(policy);
  return refs;
}

function topicKey(ref: { topic_id?: string; revision: number }) {
  return JSON.stringify([ref.topic_id, ref.revision]);
}

/** Read a complete latest-revision inventory and the exact referenced topic definitions. */
export async function backupRuntimeProfiles(
  api: ProfileTransferApi,
  tsgId: string,
  options: { profile?: string; maxPages?: number } = {},
): Promise<RuntimeProfilesBackup> {
  if (!tsgId) throw new Error('Source TSG ID is required');
  const maxPages = options.maxPages ?? 100;
  let selected = await profiles(api, maxPages);
  assertTenant(selected, tsgId);
  if (options.profile)
    selected = selected.filter(
      (p) => p.profile_id === options.profile || p.profile_name === options.profile,
    );
  if (!selected.length) throw new Error('No matching profiles to back up');
  if (options.profile && selected.length !== 1) throw new Error('Ambiguous profile selector');
  for (const profile of selected) {
    if (!profile.policy) throw new Error(`Profile has no policy: ${profile.profile_name}`);
    PolicySchema.parse(profile.policy);
  }
  const refs = selected.flatMap((profile) => topicReferences(profile.policy));
  const definitions = refs.length ? await topics(api, maxPages) : [];
  const needed = new Map<string, Topic>();
  for (const ref of refs) {
    const matches = definitions.filter((topic) => topicKey(topic) === topicKey(ref));
    if (matches.length !== 1 || matches[0].topic_name !== ref.topic_name)
      throw new Error(
        `Cannot export the exact referenced topic revision: ${ref.topic_name} revision ${ref.revision}`,
      );
    needed.set(topicKey(ref), matches[0]);
  }
  return RuntimeProfilesBackupSchema.parse({
    version: 1,
    resourceType: 'runtime-security-profiles',
    exportedAt: new Date().toISOString(),
    source: { tsgId },
    profiles: selected,
    topics: [...needed.values()],
  });
}

export interface ProfileRestoreOptions {
  namePrefix?: string;
  onConflict?: 'error' | 'skip' | 'update';
  maxPages?: number;
  /** Explicit source DLP name → already provisioned destination DLP name. */
  dlpMap?: Record<string, string>;
}
interface TopicPlan {
  key: string;
  name: string;
  source: Topic;
  existing?: Topic;
}
interface ProfilePlan {
  name: string;
  source: SecurityProfile;
  existing?: SecurityProfile;
  action: 'create' | 'update' | 'skip';
}
export interface ProfileRestorePlan {
  sourceTsgId: string;
  destinationTsgId: string;
  profiles: ProfilePlan[];
  topics: TopicPlan[];
  dlpMappings: Array<{ source: string; destination: string; id: string; version?: number }>;
  maxPages: number;
}

function sameTopic(a: Topic, b: Topic): boolean {
  return (
    a.description === b.description &&
    JSON.stringify(a.examples) === JSON.stringify(b.examples) &&
    (a.active ?? true) === (b.active ?? true)
  );
}

function dlpReferences(
  policy: unknown,
): { value: ObjectValue; name: string; kind: 'member' | 'embedded' }[] {
  if (!object(policy)) return [];
  const refs: { value: ObjectValue; name: string; kind: 'member' | 'embedded' }[] = [];
  for (const entry of Array.isArray(policy['dlp-data-profiles'])
    ? policy['dlp-data-profiles']
    : []) {
    if (!object(entry) || typeof entry.name !== 'string')
      throw new Error('Invalid embedded DLP reference');
    refs.push({ value: entry, name: entry.name, kind: 'embedded' });
  }
  for (const entry of Array.isArray(policy['ai-security-profiles'])
    ? policy['ai-security-profiles']
    : []) {
    if (!object(entry) || !object(entry['model-configuration'])) continue;
    const protection = entry['model-configuration']['data-protection'];
    if (!object(protection) || !object(protection['data-leak-detection'])) continue;
    const members = protection['data-leak-detection'].member;
    if (!Array.isArray(members)) continue;
    for (const member of members) {
      if (!object(member) || typeof member.text !== 'string') throw new Error('Invalid DLP member');
      refs.push({ value: member, name: member.text, kind: 'member' });
    }
  }
  return refs;
}

/** Refuse unrecognized tenant-bound references instead of silently transplanting IDs. */
function assertKnownPolicyReferences(policy: unknown): void {
  const known = new Map<object, string[]>([
    ...topicReferences(policy).map((ref): [object, string[]] => [ref, ['topic_id']]),
    ...dlpReferences(policy).map((ref): [object, string[]] => [
      ref.value,
      ref.kind === 'embedded' ? ['id', 'uuid'] : ['id'],
    ]),
  ]);
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
        !known.get(value)?.includes(key)
      )
        throw new Error(`Unsupported cross-tenant policy reference at ${path}.${key}`);
      visit(child, `${path}.${key}`);
    }
  };
  visit(policy, 'policy');
}

async function dataProfiles(
  api: ProfileTransferApi,
  maxPages: number,
): Promise<DataProfileResponse[]> {
  const items: DataProfileResponse[] = [];
  for (let page = 0; page < maxPages; page++) {
    const result = await api.dataProfiles.list({ page, size: 100 });
    items.push(...result.content);
    if (items.length > 10000) throw new Error('DLP inventory exceeds the safety limit');
    const last =
      result.last ??
      (result.totalPages !== undefined
        ? page + 1 >= result.totalPages
        : result.content.length < 100);
    if (last) return items;
    if (!result.content.length) throw new Error('DLP inventory pagination did not advance');
  }
  throw new Error('DLP inventory is incomplete; increase --max-pages');
}

/** Validate the entire restore and resolve identities before any mutation. */
export async function planRuntimeProfilesRestore(
  api: ProfileTransferApi,
  input: unknown,
  destinationTsgId: string,
  options: ProfileRestoreOptions = {},
): Promise<ProfileRestorePlan> {
  const backup = RuntimeProfilesBackupSchema.parse(input);
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
  if (!['error', 'skip', 'update'].includes(conflict)) throw new Error('Invalid conflict policy');
  const selected = latestProfiles(backup.profiles);
  assertTenant(selected, backup.source.tsgId);
  if (selected.length !== backup.profiles.length)
    throw new Error('Backup contains duplicate profile names/revisions');
  const crossTenant = backup.source.tsgId !== destinationTsgId;
  for (const profile of selected) {
    if (!profile.policy) throw new Error('Backup profile has no policy');
    CreateSecurityProfileRequestSchema.parse({
      profile_name: `${prefix}${profile.profile_name}`,
      active: profile.active,
      policy: profile.policy,
    });
    if (crossTenant) assertKnownPolicyReferences(profile.policy);
  }
  const existing = await profiles(api, maxPages);
  assertTenant(existing, destinationTsgId);
  const profilePlans = selected.map((source): ProfilePlan => {
    const name = `${prefix}${source.profile_name}`;
    const match = existing.find((p) => p.profile_name === name);
    if (match && conflict === 'error')
      throw new Error(
        `Destination profile already exists: ${name}; choose --on-conflict skip or update`,
      );
    if (match && !match.profile_id) throw new Error('Destination profile has no identity');
    return {
      source: structuredClone(source),
      name,
      existing: match,
      action: match ? (conflict === 'skip' ? 'skip' : 'update') : 'create',
    };
  });
  const changing = profilePlans.filter((p) => p.action !== 'skip');
  const refs = changing.flatMap((p) => topicReferences(p.source.policy));
  const targetTopics = refs.length ? await topics(api, maxPages) : [];
  const topicPlans = new Map<string, TopicPlan>();
  const topicNames = new Map<string, string>();
  for (const ref of refs) {
    const key = topicKey(ref);
    if (topicPlans.has(key)) continue;
    const definitions = backup.topics.filter(
      (topic) => topicKey(topic) === key && topic.topic_name === ref.topic_name,
    );
    if (definitions.length !== 1)
      throw new Error(`Backup lacks an unambiguous topic definition: ${ref.topic_name}`);
    const source = definitions[0];
    const name = `${prefix}${source.topic_name}`;
    assertName(name);
    if (topicNames.has(name) && topicNames.get(name) !== key)
      throw new Error(
        `Multiple source revisions use topic name ${name}; split the backup by profile/revision before restoring`,
      );
    topicNames.set(name, key);
    const matches = targetTopics
      .filter((t) => t.topic_name === name)
      .sort((a, b) => b.revision - a.revision);
    const match = matches[0];
    if (new Set(matches.map((t) => t.topic_id)).size > 1)
      throw new Error(`Ambiguous destination topic name: ${name}`);
    if (match && (!match.topic_id || !sameTopic(source, match)))
      throw new Error(
        `Destination topic differs: ${name}; use --name-prefix to avoid changing a shared topic`,
      );
    topicPlans.set(key, { key, name, source, existing: match });
  }
  const dlpRefs = crossTenant ? changing.flatMap((p) => dlpReferences(p.source.policy)) : [];
  const names = [...new Set(dlpRefs.map((ref) => ref.name))];
  for (const name of names)
    if (!options.dlpMap?.[name])
      throw new Error(
        `Cross-tenant DLP reference requires explicit --dlp-map ${name}=<destination-name>; DLP resources are not cloned`,
      );
  const catalog = names.length ? await dataProfiles(api, maxPages) : [];
  const dlpMappings = names.map((name) => {
    const target = options.dlpMap?.[name];
    const matches = catalog.filter((p) => p.name === target && p.profile_status !== 'deleted');
    if (matches.length !== 1 || !matches[0].id)
      throw new Error(`Missing or ambiguous destination DLP profile: ${target}`);
    if (
      dlpRefs.some((ref) => ref.name === name && 'version' in ref.value) &&
      matches[0].version == null
    )
      throw new Error(`Destination DLP version is unknown: ${target}`);
    return {
      source: name,
      destination: String(target),
      id: matches[0].id,
      version: matches[0].version ?? undefined,
    };
  });
  return {
    sourceTsgId: backup.source.tsgId,
    destinationTsgId,
    profiles: profilePlans,
    topics: [...topicPlans.values()],
    dlpMappings,
    maxPages,
  };
}

export interface ProfileRestoreResult {
  sourceTsgId: string;
  destinationTsgId: string;
  complete: boolean;
  topics: Array<{ name: string; action: 'created' | 'reused'; id: string }>;
  profiles: Array<{ name: string; action: 'created' | 'updated' | 'skipped'; id?: string }>;
  error?: string;
}

function canonical(value: unknown): string {
  const order = (v: unknown): unknown =>
    Array.isArray(v)
      ? v.map(order)
      : object(v)
        ? Object.fromEntries(
            Object.entries(v)
              .sort(([a], [b]) => a.localeCompare(b))
              .map(([k, child]) => [k, order(child)]),
          )
        : v;
  return JSON.stringify(order(value));
}

function unchanged(current: SecurityProfile | undefined, prior: SecurityProfile): boolean {
  return Boolean(
    current &&
      current.profile_id === prior.profile_id &&
      current.revision === prior.revision &&
      current.active === prior.active &&
      canonical(current.policy) === canonical(prior.policy),
  );
}

/** Execute a bound plan sequentially; never update shared topics or silently roll back writes. */
export async function restoreRuntimeProfiles(
  api: ProfileTransferApi,
  plan: ProfileRestorePlan,
): Promise<ProfileRestoreResult> {
  const result: ProfileRestoreResult = {
    sourceTsgId: plan.sourceTsgId,
    destinationTsgId: plan.destinationTsgId,
    complete: false,
    topics: [],
    profiles: [],
  };
  try {
    // Recheck after confirmation, before creating even a dependent topic.
    const latest = await profiles(api, plan.maxPages);
    assertTenant(latest, plan.destinationTsgId);
    if (plan.dlpMappings.length) {
      const catalog = await dataProfiles(api, plan.maxPages);
      for (const mapping of plan.dlpMappings) {
        const matches = catalog.filter(
          (p) => p.name === mapping.destination && p.profile_status !== 'deleted',
        );
        if (
          matches.length !== 1 ||
          matches[0].id !== mapping.id ||
          (matches[0].version ?? undefined) !== mapping.version
        )
          throw new Error(`Destination DLP profile changed after planning: ${mapping.destination}`);
      }
    }
    for (const item of plan.profiles) {
      if (item.action === 'skip') continue;
      const current = latest.find((p) => p.profile_name === item.name);
      if (item.existing ? !unchanged(current, item.existing) : Boolean(current))
        throw new Error(`Destination profile changed after planning: ${item.name}`);
    }
    const mappedTopics = new Map<string, Topic>();
    for (const item of plan.topics) {
      let topic = item.existing;
      const inventory = await topics(api, plan.maxPages);
      if (topic) {
        const current = inventory.find(
          (t) => t.topic_id === topic?.topic_id && t.revision === topic?.revision,
        );
        if (!current || !sameTopic(current, item.source))
          throw new Error(`Destination topic changed after planning: ${item.name}`);
      } else {
        if (inventory.some((t) => t.topic_name === item.name))
          throw new Error(`Destination topic changed after planning: ${item.name}`);
        topic = await api.topics.create({
          topic_name: item.name,
          description: item.source.description,
          examples: item.source.examples,
          active: item.source.active,
        });
      }
      if (
        !topic.topic_id ||
        topic.topic_name !== item.name ||
        !Number.isSafeInteger(topic.revision) ||
        !sameTopic(topic, item.source)
      )
        throw new Error(`Topic response did not verify: ${item.name}`);
      mappedTopics.set(item.key, topic);
      result.topics.push({
        name: item.name,
        id: topic.topic_id,
        action: item.existing ? 'reused' : 'created',
      });
    }
    for (const item of plan.profiles) {
      if (item.action === 'skip') {
        result.profiles.push({ name: item.name, action: 'skipped', id: item.existing?.profile_id });
        continue;
      }
      const policy = structuredClone(item.source.policy);
      for (const ref of topicReferences(policy)) {
        const topic = mappedTopics.get(topicKey(ref));
        if (!topic?.topic_id) throw new Error('Missing planned topic mapping');
        ref.topic_id = topic.topic_id;
        ref.topic_name = topic.topic_name;
        ref.revision = topic.revision;
      }
      for (const ref of dlpReferences(policy)) {
        const mapping = plan.dlpMappings.find((m) => m.source === ref.name);
        if (!mapping) continue;
        if (ref.kind === 'embedded') {
          ref.value.name = mapping.destination;
          ref.value.uuid = mapping.id;
        } else ref.value.text = mapping.destination;
        if ('id' in ref.value) ref.value.id = mapping.id;
        if ('version' in ref.value) {
          if (mapping.version === undefined) throw new Error('Destination DLP version is unknown');
          ref.value.version = String(mapping.version);
        }
      }
      const body = CreateSecurityProfileRequestSchema.parse({
        profile_name: item.name,
        active: item.source.active,
        policy,
      });
      const inventory = await profiles(api, plan.maxPages);
      assertTenant(inventory, plan.destinationTsgId);
      const current = inventory.find((p) => p.profile_name === item.name);
      if (item.existing ? !unchanged(current, item.existing) : Boolean(current))
        throw new Error(`Destination profile changed after planning: ${item.name}`);
      const response =
        item.action === 'update' && item.existing?.profile_id
          ? await api.profiles.update(item.existing.profile_id, body)
          : await api.profiles.create(body);
      result.profiles.push({
        name: item.name,
        action: item.action === 'create' ? 'created' : 'updated',
        id: response.profile_id,
      });
      const verified = (await profiles(api, plan.maxPages)).find(
        (p) => p.profile_name === item.name,
      );
      if (verified) assertTenant([verified], plan.destinationTsgId);
      if (
        !verified?.profile_id ||
        (response.profile_id && response.profile_id !== verified.profile_id) ||
        (item.source.active !== undefined && verified.active !== item.source.active) ||
        canonical(verified.policy) !== canonical(policy)
      )
        throw new Error(`Restored profile did not verify: ${item.name}`);
    }
    result.complete = true;
  } catch (error) {
    // SDK errors can include request/response content; publish only safe local messages or status.
    const status = (error as { statusCode?: number })?.statusCode;
    result.error = Number.isInteger(status)
      ? `API request failed (HTTP ${status}); verify destination state before retrying`
      : error instanceof Error &&
          /^(Destination|Topic response|Missing planned|Restored profile|Inventory|DLP inventory)/.test(
            error.message,
          )
        ? error.message
        : 'Restore stopped; verify destination state before retrying';
  }
  return result;
}
