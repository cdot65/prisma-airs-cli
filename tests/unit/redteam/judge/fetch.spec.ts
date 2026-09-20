import { describe, expect, it, vi } from 'vitest';
import {
  type AttackReportsClient,
  fetchJobAttackRecords,
  normalizeScan,
} from '../../../../src/redteam/judge/index.js';

function client(total: number, pageSize: number): AttackReportsClient & { calls: number[] } {
  const ids = Array.from({ length: total }, (_, i) => `attack-${i}`);
  const calls: number[] = [];
  return {
    calls,
    async listAttacks(_jobId, opts) {
      calls.push(opts?.skip ?? 0);
      const skip = opts?.skip ?? 0;
      return {
        pagination: { total_items: total },
        data: ids.slice(skip, skip + (opts?.limit ?? pageSize)).map((uuid) => ({ uuid })),
      };
    },
    async getAttackDetail(_jobId, attackId) {
      return {
        uuid: attackId,
        job_id: 'job-1',
        target_id: 't',
        prompt: `prompt ${attackId}`,
        category: 'SECURITY',
        sub_category: 'JAILBREAK',
        category_display_name: 'Security',
        sub_category_display_name: 'Jailbreak',
        goal: attackId === 'attack-0' ? 'leak' : null,
        severity: 'HIGH',
        asr: 50,
        threat: true,
        outputs: [
          { uuid: `${attackId}-out`, output: 'leaked', threat: true, error: false },
          { uuid: `${attackId}-err`, output: '', error: true, error_message: 'timeout' },
        ],
      };
    },
  };
}

describe('fetchJobAttackRecords', () => {
  it('pages through the attack list and maps details into judge records', async () => {
    const fake = client(7, 3);
    const records = await fetchJobAttackRecords(fake, 'job-1', { pageSize: 3, concurrency: 2 });
    expect(fake.calls).toEqual([0, 3, 6]);
    expect(records).toHaveLength(7);
    expect(records[0]).toEqual({
      uuid: 'attack-0',
      job_id: 'job-1',
      target_id: 't',
      prompt: 'prompt attack-0',
      category: 'SECURITY',
      sub_category: 'JAILBREAK',
      category_display_name: 'Security',
      sub_category_display_name: 'Jailbreak',
      goal: 'leak',
      severity: 'HIGH',
      asr: 50,
      threat: true,
      marked_safe: null,
      error: null,
      attack_modality: null,
      multi_turn: null,
      outputs: [
        {
          uuid: 'attack-0-out',
          output: 'leaked',
          threat: true,
          marked_safe: null,
          error: false,
          error_message: null,
        },
        {
          uuid: 'attack-0-err',
          output: '',
          threat: null,
          marked_safe: null,
          error: true,
          error_message: 'timeout',
        },
      ],
    });
    const { units, notes } = normalizeScan(records);
    expect(notes).toEqual({
      layout: 'array',
      records: 7,
      skipped_no_prompt: 0,
      error_outputs: 7,
      objective_proxies: 6,
    });
    expect(units[0].unit_id).toBe('attack-0#attack-0-out');
    expect(units[0].objective).toBe('leak');
    expect(units[2].objective).toBe(
      'Security / Jailbreak attack: make the target do what the prompt asks.',
    );
  });

  it('rejects duplicate pages before fetching details', async () => {
    const listAttacks = vi.fn().mockResolvedValue({ data: [{ uuid: 'a' }, { uuid: 'a' }] });
    const getAttackDetail = vi.fn();
    await expect(
      fetchJobAttackRecords({ listAttacks, getAttackDetail }, 'job', { pageSize: 2 }),
    ).rejects.toThrow('duplicate');
    expect(getAttackDetail).not.toHaveBeenCalled();
  });

  it('continues beyond a page-sized total and tolerates missing outputs', async () => {
    const listAttacks = vi
      .fn()
      .mockResolvedValueOnce({
        pagination: { total_items: 2 },
        data: [{ uuid: 'a' }, { uuid: 'b' }],
      })
      .mockResolvedValueOnce({ pagination: { total_items: 1 }, data: [{ uuid: 'c' }] });
    const getAttackDetail = vi.fn(async (_job: string, uuid: string) => ({ uuid, prompt: 'p' }));
    const records = await fetchJobAttackRecords({ listAttacks, getAttackDetail }, 'job', {
      pageSize: 2,
    });
    expect(records).toHaveLength(3);
    expect(listAttacks).toHaveBeenCalledTimes(2);
    expect(normalizeScan(records).units[0]).toMatchObject({ unit_id: 'a#0', is_error: true });
  });

  it('keeps paging while pages are full and no total is reported', async () => {
    const listAttacks = vi
      .fn()
      .mockResolvedValueOnce({ data: [{ uuid: 'a' }, { uuid: 'b' }] })
      .mockResolvedValueOnce({ data: [{ uuid: 'c' }] });
    const getAttackDetail = vi.fn(async (_job: string, uuid: string) => ({ uuid, prompt: 'p' }));
    const records = await fetchJobAttackRecords({ listAttacks, getAttackDetail }, 'job', {
      pageSize: 2,
    });
    expect(listAttacks).toHaveBeenCalledTimes(2);
    expect(records.map((r) => r.uuid)).toEqual(['a', 'b', 'c']);
  });

  it('propagates AIRS failures so the run stops before spending judge budget', async () => {
    const failing: AttackReportsClient = {
      listAttacks: async () => ({ data: [{ uuid: 'a' }] }),
      getAttackDetail: async () => {
        throw Object.assign(new Error('Forbidden'), { status: 403 });
      },
    };
    await expect(fetchJobAttackRecords(failing, 'job')).rejects.toMatchObject({ status: 403 });
  });
});
