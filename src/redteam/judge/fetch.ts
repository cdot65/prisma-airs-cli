import pLimit from 'p-limit';

/**
 * The slice of `RedTeamClient.reports` the judge needs. Both responses are the SDK's
 * passthrough Zod objects, so only the fields declared in the SDK schemas are mapped.
 */
export interface AttackReportsClient {
  listAttacks(
    jobId: string,
    opts?: { limit?: number; skip?: number },
  ): Promise<{ pagination?: { total_items?: number | null }; data: Array<{ uuid: string }> }>;
  getAttackDetail(jobId: string, attackId: string): Promise<AttackDetailLike>;
}

/** Fields of `AttackDetailResponseSchema` the judge reads. */
export interface AttackDetailLike {
  uuid: string;
  job_id?: string;
  target_id?: string;
  prompt: string;
  category?: string;
  sub_category?: string;
  category_display_name?: string;
  sub_category_display_name?: string;
  goal?: string | null;
  severity?: string;
  asr?: number | null;
  threat?: boolean | null;
  marked_safe?: boolean | null;
  error?: boolean | null;
  attack_modality?: string;
  multi_turn?: boolean;
  outputs?: Array<{
    uuid: string;
    output: string;
    threat?: boolean | null;
    marked_safe?: boolean | null;
    error?: boolean | null;
    error_message?: string | null;
  }>;
}

/** One record in the shape `normalizeScan` accepts (API detail layout with `outputs[]`). */
export interface JobAttackRecord {
  uuid: string;
  job_id: string | null;
  target_id: string | null;
  prompt: string;
  category: string | null;
  sub_category: string | null;
  category_display_name: string | null;
  sub_category_display_name: string | null;
  goal: string | null;
  severity: string | null;
  asr: number | null;
  threat: boolean | null;
  marked_safe: boolean | null;
  error: boolean | null;
  attack_modality: string | null;
  multi_turn: boolean | null;
  /** Present only when the detail carried `outputs[]`; absent details yield one skipped unit. */
  outputs?: Array<{
    uuid: string;
    output: string;
    threat: boolean | null;
    marked_safe: boolean | null;
    error: boolean | null;
    error_message: string | null;
  }>;
}

export const JOB_PAGE_SIZE = 50;
/** Hard stop so a misbehaving pagination response can never loop forever. */
const MAX_PAGES = 1000;

function toRecord(detail: AttackDetailLike): JobAttackRecord {
  const record: JobAttackRecord = {
    uuid: detail.uuid,
    job_id: detail.job_id ?? null,
    target_id: detail.target_id ?? null,
    prompt: detail.prompt,
    category: detail.category ?? null,
    sub_category: detail.sub_category ?? null,
    category_display_name: detail.category_display_name ?? null,
    sub_category_display_name: detail.sub_category_display_name ?? null,
    goal: detail.goal ?? null,
    severity: detail.severity ?? null,
    asr: detail.asr ?? null,
    threat: detail.threat ?? null,
    marked_safe: detail.marked_safe ?? null,
    error: detail.error ?? null,
    attack_modality: detail.attack_modality ?? null,
    multi_turn: detail.multi_turn ?? null,
  };
  if (Array.isArray(detail.outputs))
    record.outputs = detail.outputs.map((output) => ({
      uuid: output.uuid,
      output: output.output,
      threat: output.threat ?? null,
      marked_safe: output.marked_safe ?? null,
      error: output.error ?? null,
      error_message: output.error_message ?? null,
    }));
  return record;
}

/**
 * Page through a job's attacks, then fetch every attack detail (which carries `goal`
 * and the per-output verdicts) with bounded concurrency. Any AIRS failure rejects so the
 * run stops before spending judge budget on an incomplete scan.
 */
export async function fetchJobAttackRecords(
  client: AttackReportsClient,
  jobId: string,
  options: { concurrency?: number; pageSize?: number } = {},
): Promise<JobAttackRecord[]> {
  const pageSize = options.pageSize ?? JOB_PAGE_SIZE;
  const ids: string[] = [];
  const seen = new Set<string>();
  for (let page = 0; page < MAX_PAGES; page += 1) {
    const response = await client.listAttacks(jobId, { limit: pageSize, skip: page * pageSize });
    const data = Array.isArray(response.data) ? response.data : [];
    for (const item of data) {
      if (typeof item.uuid !== 'string' || seen.has(item.uuid))
        throw new Error('Attack pagination returned an invalid or duplicate identifier');
      if (typeof item.uuid === 'string') {
        seen.add(item.uuid);
        ids.push(item.uuid);
      }
    }
    if (data.length < pageSize) break;
    if (page === MAX_PAGES - 1)
      throw new Error('Attack pagination exceeded its bound; refusing to judge an incomplete scan');
  }
  const limit = pLimit(Math.max(1, options.concurrency ?? 5));
  return Promise.all(
    ids.map((uuid) => limit(async () => toRecord(await client.getAttackDetail(jobId, uuid)))),
  );
}
