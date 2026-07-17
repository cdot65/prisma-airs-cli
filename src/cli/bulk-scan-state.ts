import { randomUUID } from 'node:crypto';
import * as fs from 'node:fs/promises';
import * as path from 'node:path';
import { z } from 'zod';
import type { BulkScanResult } from '../airs/types.js';

export type BulkScanItemStatus =
  | 'pending'
  | 'submitting'
  | 'submitted'
  | 'complete'
  | 'failed'
  | 'ambiguous';

/** Durable state for one input prompt throughout the async scan lifecycle. */
export interface BulkScanItemState {
  index: number;
  reqId: number;
  prompt: string;
  status: BulkScanItemStatus;
  scanId?: string;
  receiptReportId?: string;
  result?: BulkScanResult;
  error?: string;
}

/** Versioned, resumable bulk-scan job state. */
export interface BulkScanState {
  version: 2;
  profile: string;
  sessionId?: string;
  outputFile: string;
  batchSize: number;
  createdAt: string;
  updatedAt: string;
  items: BulkScanItemState[];
}

const BulkScanResultSchema = z.object({
  index: z.number().int().nonnegative(),
  reqId: z.number().int().nonnegative(),
  prompt: z.string(),
  response: z.string().optional(),
  scanId: z.string(),
  reportId: z.string(),
  action: z.enum(['allow', 'block', 'failed']),
  category: z.string(),
  triggered: z.boolean(),
  detections: z.record(z.boolean()),
  error: z.string().optional(),
});

const BulkScanItemSchema = z
  .object({
    index: z.number().int().nonnegative(),
    reqId: z.number().int().nonnegative(),
    prompt: z.string(),
    status: z.enum(['pending', 'submitting', 'submitted', 'complete', 'failed', 'ambiguous']),
    scanId: z.string().min(1).optional(),
    receiptReportId: z.string().optional(),
    result: BulkScanResultSchema.optional(),
    error: z.string().optional(),
  })
  .superRefine((item, ctx) => {
    if (item.reqId !== item.index) {
      ctx.addIssue({ code: 'custom', message: 'reqId must match the stable input index' });
    }
    if (['submitted', 'complete', 'failed'].includes(item.status) && !item.scanId) {
      ctx.addIssue({ code: 'custom', message: `${item.status} entries require a scanId` });
    }
    if (['complete', 'failed'].includes(item.status) && !item.result) {
      ctx.addIssue({ code: 'custom', message: `${item.status} entries require a result` });
    }
    if (!['complete', 'failed'].includes(item.status) && item.result) {
      ctx.addIssue({ code: 'custom', message: `${item.status} entries cannot contain a result` });
    }
    if (['pending', 'submitting', 'ambiguous'].includes(item.status) && item.scanId) {
      ctx.addIssue({ code: 'custom', message: `${item.status} entries cannot contain a scanId` });
    }
    if (item.result) {
      if (
        item.result.index !== item.index ||
        item.result.reqId !== item.reqId ||
        item.result.prompt !== item.prompt
      ) {
        ctx.addIssue({ code: 'custom', message: 'stored result does not match its prompt entry' });
      }
      if (item.result.scanId !== item.scanId) {
        ctx.addIssue({
          code: 'custom',
          message: 'stored result scanId does not match its receipt',
        });
      }
      if (item.status === 'failed' && item.result.action !== 'failed') {
        ctx.addIssue({ code: 'custom', message: 'failed entries require a failed result' });
      }
      if (item.status === 'complete' && item.result.action === 'failed') {
        ctx.addIssue({
          code: 'custom',
          message: 'complete entries cannot contain a failed result',
        });
      }
    }
  });

const BulkScanStateSchema = z
  .object({
    version: z.literal(2),
    profile: z.string().min(1),
    sessionId: z.string().optional(),
    outputFile: z.string().min(1),
    batchSize: z.number().int().positive().max(Number.MAX_SAFE_INTEGER),
    createdAt: z.string().datetime(),
    updatedAt: z.string().datetime(),
    items: z.array(BulkScanItemSchema).min(1),
  })
  .superRefine((state, ctx) => {
    const indices = new Set<number>();
    for (const [position, item] of state.items.entries()) {
      if (indices.has(item.index)) {
        ctx.addIssue({ code: 'custom', message: `duplicate input index ${item.index}` });
      }
      if (item.index !== position) {
        ctx.addIssue({ code: 'custom', message: 'prompt entries must remain in input order' });
      }
      indices.add(item.index);
    }
    const sorted = [...indices].sort((left, right) => left - right);
    if (sorted.some((index, position) => index !== position)) {
      ctx.addIssue({ code: 'custom', message: 'input indices must be contiguous from zero' });
    }
  });

/** Persist a bulk-scan job, rewriting the same file when `filePath` is supplied. */
export async function saveBulkScanState(
  state: BulkScanState,
  dir: string,
  filePath?: string,
): Promise<string> {
  state.updatedAt = new Date().toISOString();
  const validation = BulkScanStateSchema.safeParse(state);
  if (!validation.success) {
    const reason = validation.error.issues.map((issue) => issue.message).join('; ');
    throw new Error(`Invalid bulk-scan state: ${reason}`);
  }
  await fs.mkdir(dir, { recursive: true, mode: 0o700 });
  if (!filePath) await fs.chmod(dir, 0o700);
  const target =
    filePath ??
    path.join(dir, `${state.createdAt.replace(/[:.]/g, '-')}-${randomUUID()}.bulk-scan.json`);
  const temporary = `${target}.tmp-${process.pid}-${randomUUID()}`;
  try {
    await fs.writeFile(temporary, JSON.stringify(state, null, 2), {
      encoding: 'utf-8',
      flag: 'wx',
      mode: 0o600,
    });
    await fs.rename(temporary, target);
    await fs.chmod(target, 0o600);
  } catch (error) {
    await fs.rm(temporary, { force: true });
    throw error;
  }
  return target;
}

/** Load a current bulk-scan state file; legacy state cannot restore prompt correlation. */
export async function loadBulkScanState(filePath: string): Promise<BulkScanState> {
  const raw = await fs.readFile(filePath, 'utf-8');
  let parsed: unknown;
  try {
    parsed = JSON.parse(raw);
  } catch {
    throw new Error('Invalid bulk-scan state: malformed JSON');
  }
  if ((parsed as { version?: unknown })?.version !== 2) {
    throw new Error(
      'This legacy bulk-scan state predates prompt persistence and cannot be resumed. Re-run bulk-scan.',
    );
  }
  const result = BulkScanStateSchema.safeParse(parsed);
  if (!result.success) {
    const reason = result.error.issues.map((issue) => issue.message).join('; ');
    throw new Error(`Invalid bulk-scan state: ${reason}`);
  }
  return result.data as BulkScanState;
}
