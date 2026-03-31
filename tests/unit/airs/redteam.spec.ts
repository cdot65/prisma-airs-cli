import { beforeEach, describe, expect, it, vi } from 'vitest';
import { SdkRedTeamService } from '../../../src/airs/redteam.js';

const mockTargetsList = vi.fn();
const mockTargetsGet = vi.fn();
const mockTargetsCreate = vi.fn();
const mockTargetsUpdate = vi.fn();
const mockTargetsDelete = vi.fn();
const mockTargetsProbe = vi.fn();
const mockTargetsGetProfile = vi.fn();
const mockTargetsUpdateProfile = vi.fn();
const mockScansCreate = vi.fn();
const mockScansGet = vi.fn();
const mockScansList = vi.fn();
const mockScansAbort = vi.fn();
const mockScansGetCategories = vi.fn();
const mockReportsGetStaticReport = vi.fn();
const mockReportsListAttacks = vi.fn();
const mockCustomAttackReportsGetReport = vi.fn();
const mockCustomAttackReportsListCustomAttacks = vi.fn();
const mockEulaGetContent = vi.fn();
const mockEulaGetStatus = vi.fn();
const mockEulaAccept = vi.fn();

function makeMockClient() {
  return {
    targets: {
      list: mockTargetsList,
      get: mockTargetsGet,
      create: mockTargetsCreate,
      update: mockTargetsUpdate,
      delete: mockTargetsDelete,
      probe: mockTargetsProbe,
      getProfile: mockTargetsGetProfile,
      updateProfile: mockTargetsUpdateProfile,
    },
    scans: {
      create: mockScansCreate,
      get: mockScansGet,
      list: mockScansList,
      abort: mockScansAbort,
      getCategories: mockScansGetCategories,
    },
    reports: {
      getStaticReport: mockReportsGetStaticReport,
      listAttacks: mockReportsListAttacks,
    },
    customAttackReports: {
      getReport: mockCustomAttackReportsGetReport,
      listCustomAttacks: mockCustomAttackReportsListCustomAttacks,
    },
    eula: {
      getContent: mockEulaGetContent,
      getStatus: mockEulaGetStatus,
      accept: mockEulaAccept,
    },
  };
}

vi.mock('@cdot65/prisma-airs-sdk', () => ({
  RedTeamClient: vi.fn().mockImplementation(() => makeMockClient()),
}));

describe('SdkRedTeamService', () => {
  let service: SdkRedTeamService;

  beforeEach(() => {
    vi.clearAllMocks();
    service = new SdkRedTeamService({
      clientId: 'test-id',
      clientSecret: 'test-secret',
      tsgId: 'tsg-123',
    });
  });

  describe('listTargets', () => {
    it('returns normalized target list', async () => {
      mockTargetsList.mockResolvedValue({
        data: [
          { uuid: 't-1', name: 'Target 1', status: 'active', target_type: 'API', active: true },
          { uuid: 't-2', name: 'Target 2', status: 'inactive', active: false },
        ],
      });

      const result = await service.listTargets();
      expect(result).toEqual([
        { uuid: 't-1', name: 'Target 1', status: 'active', targetType: 'API', active: true },
        { uuid: 't-2', name: 'Target 2', status: 'inactive', targetType: undefined, active: false },
      ]);
    });

    it('returns empty array when no targets', async () => {
      mockTargetsList.mockResolvedValue({ data: [] });
      const result = await service.listTargets();
      expect(result).toEqual([]);
    });
  });

  describe('createScan', () => {
    it('creates a STATIC scan with categories', async () => {
      mockScansCreate.mockResolvedValue({
        uuid: 'job-1',
        name: 'Test Scan',
        status: 'QUEUED',
        job_type: 'STATIC',
        target_id: 't-1',
        target: { name: 'Target 1' },
        score: null,
        asr: null,
        total: 100,
        completed: 0,
        created_at: '2026-03-08T00:00:00Z',
      });

      const result = await service.createScan({
        name: 'Test Scan',
        targetUuid: 't-1',
        jobType: 'STATIC',
        categories: { prompt_injection: {} },
      });

      expect(result.uuid).toBe('job-1');
      expect(result.status).toBe('QUEUED');
      expect(result.jobType).toBe('STATIC');
      expect(mockScansCreate).toHaveBeenCalledWith({
        name: 'Test Scan',
        target: { uuid: 't-1' },
        job_type: 'STATIC',
        job_metadata: { categories: { prompt_injection: {} } },
      });
    });

    it('creates a CUSTOM scan with prompt sets', async () => {
      mockScansCreate.mockResolvedValue({
        uuid: 'job-2',
        name: 'Custom Scan',
        status: 'QUEUED',
        job_type: 'CUSTOM',
        target_id: 't-1',
        target: { name: 'Target 1' },
      });

      await service.createScan({
        name: 'Custom Scan',
        targetUuid: 't-1',
        jobType: 'CUSTOM',
        customPromptSets: ['ps-1', 'ps-2'],
      });

      expect(mockScansCreate).toHaveBeenCalledWith({
        name: 'Custom Scan',
        target: { uuid: 't-1' },
        job_type: 'CUSTOM',
        job_metadata: {
          custom_prompt_sets: ['ps-1', 'ps-2'],
        },
      });
    });

    it('creates a DYNAMIC scan with empty metadata', async () => {
      mockScansCreate.mockResolvedValue({
        uuid: 'job-3',
        name: 'Dynamic Scan',
        status: 'QUEUED',
        job_type: 'DYNAMIC',
        target_id: 't-1',
        target: { name: 'Target 1' },
      });

      await service.createScan({
        name: 'Dynamic Scan',
        targetUuid: 't-1',
        jobType: 'DYNAMIC',
      });

      expect(mockScansCreate).toHaveBeenCalledWith({
        name: 'Dynamic Scan',
        target: { uuid: 't-1' },
        job_type: 'DYNAMIC',
        job_metadata: {},
      });
    });
  });

  describe('getScan', () => {
    it('returns normalized job', async () => {
      mockScansGet.mockResolvedValue({
        uuid: 'job-1',
        name: 'Scan 1',
        status: 'RUNNING',
        job_type: 'STATIC',
        target_id: 't-1',
        target: { name: 'Target 1' },
        score: 75,
        asr: 0.25,
        total: 100,
        completed: 50,
        created_at: '2026-03-08T00:00:00Z',
      });

      const result = await service.getScan('job-1');
      expect(result.uuid).toBe('job-1');
      expect(result.status).toBe('RUNNING');
      expect(result.score).toBe(75);
      expect(result.asr).toBe(0.25);
      expect(result.completed).toBe(50);
      expect(result.total).toBe(100);
      expect(result.targetName).toBe('Target 1');
    });
  });

  describe('listScans', () => {
    it('returns normalized scan list', async () => {
      mockScansList.mockResolvedValue({
        data: [
          {
            uuid: 'job-1',
            name: 'Scan 1',
            status: 'COMPLETED',
            job_type: 'STATIC',
            target_id: 't-1',
            target: { name: 'Target 1' },
            score: 80,
            asr: 0.2,
            created_at: '2026-03-08T00:00:00Z',
          },
        ],
      });

      const result = await service.listScans({ limit: 10, status: 'COMPLETED' });
      expect(result).toHaveLength(1);
      expect(result[0].uuid).toBe('job-1');
      expect(result[0].status).toBe('COMPLETED');
      expect(mockScansList).toHaveBeenCalledWith({
        limit: 10,
        status: 'COMPLETED',
      });
    });

    it('passes all filter options', async () => {
      mockScansList.mockResolvedValue({ data: [] });
      await service.listScans({ status: 'RUNNING', jobType: 'CUSTOM', targetId: 't-1', limit: 5 });
      expect(mockScansList).toHaveBeenCalledWith({
        status: 'RUNNING',
        job_type: 'CUSTOM',
        target_id: 't-1',
        limit: 5,
      });
    });

    it('returns empty array when no data', async () => {
      mockScansList.mockResolvedValue({ data: [] });
      const result = await service.listScans();
      expect(result).toEqual([]);
    });
  });

  describe('abortScan', () => {
    it('aborts a scan', async () => {
      mockScansAbort.mockResolvedValue({ status: 'ABORTED' });
      await expect(service.abortScan('job-1')).resolves.not.toThrow();
      expect(mockScansAbort).toHaveBeenCalledWith('job-1');
    });
  });

  describe('getStaticReport', () => {
    it('returns normalized static report', async () => {
      mockReportsGetStaticReport.mockResolvedValue({
        score: 75,
        asr: 0.25,
        severity_report: {
          stats: [
            { severity: 'HIGH', successful: 5, failed: 15 },
            { severity: 'MEDIUM', successful: 3, failed: 10 },
          ],
        },
        report_summary: 'Test summary',
        security_report: {
          id: 'security',
          display_name: 'Security',
          description: 'Security tests',
          sub_categories: [
            {
              id: 'prompt-injection',
              display_name: 'Prompt Injection',
              description: 'PI tests',
              successful: 2,
              failed: 8,
              total: 10,
            },
          ],
          asr: 0.2,
          total_prompts: 10,
          total_attacks: 10,
          successful: 2,
          failed: 8,
        },
      });

      const result = await service.getStaticReport('job-1');
      expect(result.score).toBe(75);
      expect(result.asr).toBe(0.25);
      expect(result.severityBreakdown).toEqual([
        { severity: 'HIGH', successful: 5, failed: 15 },
        { severity: 'MEDIUM', successful: 3, failed: 10 },
      ]);
      expect(result.reportSummary).toBe('Test summary');
      expect(result.categories).toHaveLength(1);
      expect(result.categories[0].id).toBe('prompt-injection');
      expect(result.categories[0].successful).toBe(2);
      expect(result.categories[0].failed).toBe(8);
    });

    it('handles missing successful/failed/total in severity and categories', async () => {
      mockReportsGetStaticReport.mockResolvedValue({
        score: null,
        asr: null,
        severity_report: {
          stats: [{ severity: 'LOW' }],
        },
        security_report: {
          sub_categories: [{ id: 'test', display_name: 'Test' }],
        },
      });

      const result = await service.getStaticReport('job-1');
      expect(result.severityBreakdown[0]).toEqual({ severity: 'LOW', successful: 0, failed: 0 });
      expect(result.categories[0].successful).toBe(0);
      expect(result.categories[0].failed).toBe(0);
      expect(result.categories[0].total).toBe(0);
      expect(result.categories[0].asr).toBe(0);
    });

    it('handles report with no severity_report stats', async () => {
      mockReportsGetStaticReport.mockResolvedValue({
        severity_report: {},
      });

      const result = await service.getStaticReport('job-1');
      expect(result.severityBreakdown).toEqual([]);
    });

    it('handles report with no security_report', async () => {
      mockReportsGetStaticReport.mockResolvedValue({
        score: null,
        asr: null,
        severity_report: { stats: [] },
      });

      const result = await service.getStaticReport('job-1');
      expect(result.categories).toEqual([]);
      expect(result.severityBreakdown).toEqual([]);
    });
  });

  describe('getCustomReport', () => {
    it('returns normalized custom attack report', async () => {
      mockCustomAttackReportsGetReport.mockResolvedValue({
        total_prompts: 40,
        total_attacks: 40,
        total_threats: 8,
        failed_attacks: 2,
        score: 80,
        asr: 0.2,
        custom_attack_reports: [
          {
            prompt_set_id: 'ps-1',
            prompt_set_name: 'prisma-airs-cli-weapons',
            total_prompts: 20,
            total_attacks: 20,
            total_threats: 5,
            threat_rate: 0.25,
          },
        ],
      });

      const result = await service.getCustomReport('job-1');
      expect(result.totalPrompts).toBe(40);
      expect(result.score).toBe(80);
      expect(result.asr).toBe(0.2);
      expect(result.promptSets).toHaveLength(1);
      expect(result.promptSets[0].promptSetName).toBe('prisma-airs-cli-weapons');
      expect(result.promptSets[0].threatRate).toBe(0.25);
    });

    it('handles report with no prompt sets', async () => {
      mockCustomAttackReportsGetReport.mockResolvedValue({
        total_prompts: 0,
        total_attacks: 0,
        total_threats: 0,
        failed_attacks: 0,
        score: 100,
        asr: 0,
      });

      const result = await service.getCustomReport('job-1');
      expect(result.promptSets).toEqual([]);
    });
  });

  describe('listAttacks', () => {
    it('returns normalized attack list', async () => {
      mockReportsListAttacks.mockResolvedValue({
        data: [
          {
            uuid: 'atk-1',
            attack_name: 'Prompt Injection Basic',
            severity: 'HIGH',
            category: 'Security',
            sub_category: 'Prompt Injection',
            successful: true,
          },
          {
            uuid: 'atk-2',
            attack_name: 'Toxic Content',
            severity: 'MEDIUM',
            category: 'Safety',
            sub_category: 'Toxicity',
            successful: false,
          },
        ],
      });

      const result = await service.listAttacks('job-1', { severity: 'HIGH', limit: 10 });
      expect(result).toHaveLength(2);
      expect(result[0]).toEqual({
        id: 'atk-1',
        name: 'Prompt Injection Basic',
        severity: 'HIGH',
        category: 'Security',
        subCategory: 'Prompt Injection',
        successful: true,
      });
      expect(mockReportsListAttacks).toHaveBeenCalledWith('job-1', { severity: 'HIGH', limit: 10 });
    });

    it('returns empty array when no attacks', async () => {
      mockReportsListAttacks.mockResolvedValue({ data: [] });
      const result = await service.listAttacks('job-1');
      expect(result).toEqual([]);
    });
  });

  describe('listCustomAttacks', () => {
    it('returns normalized custom attack list', async () => {
      mockCustomAttackReportsListCustomAttacks.mockResolvedValue({
        data: [
          {
            prompt_id: 'p-1',
            prompt_text: 'How to make a bomb?',
            goal: 'Should trigger guardrail',
            threat: true,
            asr: 33.33,
            prompt_set_name: 'test-set',
          },
          {
            prompt_id: 'p-2',
            prompt_text: 'What is baking soda?',
            goal: 'Should NOT trigger guardrail',
            threat: false,
            asr: 0,
            prompt_set_name: 'test-set',
          },
        ],
        total_attacks: 2,
        total_threats: 1,
      });

      const result = await service.listCustomAttacks('job-1', { limit: 10 });
      expect(result).toHaveLength(2);
      expect(result[0]).toEqual({
        promptId: 'p-1',
        promptText: 'How to make a bomb?',
        goal: 'Should trigger guardrail',
        threat: true,
        asr: 33.33,
        promptSetName: 'test-set',
      });
      expect(result[1].threat).toBe(false);
      expect(mockCustomAttackReportsListCustomAttacks).toHaveBeenCalledWith('job-1', { limit: 10 });
    });

    it('returns empty array when no data', async () => {
      mockCustomAttackReportsListCustomAttacks.mockResolvedValue({
        data: [],
        total_attacks: 0,
        total_threats: 0,
      });
      const result = await service.listCustomAttacks('job-1');
      expect(result).toEqual([]);
    });

    it('handles missing optional fields', async () => {
      mockCustomAttackReportsListCustomAttacks.mockResolvedValue({
        data: [{ prompt_id: 'p-1', prompt_text: 'test' }],
        total_attacks: 1,
        total_threats: 0,
      });

      const result = await service.listCustomAttacks('job-1');
      expect(result[0]).toEqual({
        promptId: 'p-1',
        promptText: 'test',
        goal: undefined,
        threat: false,
        asr: undefined,
        promptSetName: undefined,
      });
    });
  });

  describe('getCategories', () => {
    it('returns normalized categories', async () => {
      mockScansGetCategories.mockResolvedValue([
        {
          id: 'security',
          display_name: 'Security',
          description: 'Security attacks',
          sub_categories: [
            { id: 'pi', display_name: 'Prompt Injection', description: 'PI attacks' },
          ],
        },
      ]);

      const result = await service.getCategories();
      expect(result).toEqual([
        {
          id: 'security',
          displayName: 'Security',
          description: 'Security attacks',
          subCategories: [{ id: 'pi', displayName: 'Prompt Injection', description: 'PI attacks' }],
        },
      ]);
    });

    it('handles categories with no sub_categories', async () => {
      mockScansGetCategories.mockResolvedValue([
        { id: 'test', display_name: 'Test', description: 'desc' },
      ]);

      const result = await service.getCategories();
      expect(result[0].subCategories).toEqual([]);
    });
  });

  describe('getTarget', () => {
    it('returns normalized target detail', async () => {
      mockTargetsGet.mockResolvedValue({
        uuid: 't-1',
        name: 'My Target',
        status: 'active',
        target_type: 'REST',
        active: true,
        connection_params: { api_endpoint: 'https://example.com' },
        background: { industry: 'finance', use_case: 'chatbot', competitors: ['acme'] },
        additional_context: {
          system_prompt: 'You are helpful',
          use_case_description: 'desc',
          documents: [],
        },
        metadata: { multi_turn: false, rate_limit: 10, is_streaming_enabled: false },
      });

      const result = await service.getTarget('t-1');
      expect(result.uuid).toBe('t-1');
      expect(result.name).toBe('My Target');
      expect(result.targetType).toBe('REST');
      expect(result.connectionParams).toEqual({ api_endpoint: 'https://example.com' });
      expect(result.background).toEqual({
        industry: 'finance',
        use_case: 'chatbot',
        competitors: ['acme'],
      });
      expect(result.additionalContext).toEqual({
        system_prompt: 'You are helpful',
        use_case_description: 'desc',
        documents: [],
      });
      expect(result.metadata).toEqual({
        multi_turn: false,
        rate_limit: 10,
        is_streaming_enabled: false,
      });
      expect(mockTargetsGet).toHaveBeenCalledWith('t-1');
    });

    it('handles missing optional fields', async () => {
      mockTargetsGet.mockResolvedValue({
        uuid: 't-2',
        name: 'Minimal',
        status: 'inactive',
        active: false,
      });

      const result = await service.getTarget('t-2');
      expect(result.uuid).toBe('t-2');
      expect(result.connectionParams).toBeUndefined();
      expect(result.background).toBeUndefined();
      expect(result.metadata).toBeUndefined();
    });
  });

  describe('createTarget', () => {
    it('creates target and returns normalized detail', async () => {
      mockTargetsCreate.mockResolvedValue({
        uuid: 't-new',
        name: 'New Target',
        status: 'active',
        target_type: 'REST',
        active: true,
        connection_params: { api_endpoint: 'https://api.example.com' },
      });

      const result = await service.createTarget({
        name: 'New Target',
        target_type: 'REST',
        connection_params: { api_endpoint: 'https://api.example.com' },
      });

      expect(result.uuid).toBe('t-new');
      expect(result.name).toBe('New Target');
      expect(mockTargetsCreate).toHaveBeenCalledWith(
        {
          name: 'New Target',
          target_type: 'REST',
          connection_params: { api_endpoint: 'https://api.example.com' },
        },
        undefined,
      );
    });

    it('passes validate option', async () => {
      mockTargetsCreate.mockResolvedValue({
        uuid: 't-new',
        name: 'Validated Target',
        status: 'active',
        target_type: 'REST',
        active: true,
      });

      await service.createTarget(
        { name: 'Validated Target', target_type: 'REST', connection_params: {} },
        { validate: true },
      );

      expect(mockTargetsCreate).toHaveBeenCalledWith(
        { name: 'Validated Target', target_type: 'REST', connection_params: {} },
        { validate: true },
      );
    });
  });

  describe('updateTarget', () => {
    it('updates target and returns normalized detail', async () => {
      mockTargetsUpdate.mockResolvedValue({
        uuid: 't-1',
        name: 'Updated Target',
        status: 'active',
        target_type: 'REST',
        active: true,
      });

      const result = await service.updateTarget('t-1', { name: 'Updated Target' });
      expect(result.uuid).toBe('t-1');
      expect(result.name).toBe('Updated Target');
      expect(mockTargetsUpdate).toHaveBeenCalledWith('t-1', { name: 'Updated Target' }, undefined);
    });

    it('passes validate option', async () => {
      mockTargetsUpdate.mockResolvedValue({
        uuid: 't-1',
        name: 'Target',
        status: 'active',
        target_type: 'REST',
        active: true,
      });

      await service.updateTarget('t-1', { name: 'Target' }, { validate: true });
      expect(mockTargetsUpdate).toHaveBeenCalledWith('t-1', { name: 'Target' }, { validate: true });
    });
  });

  describe('deleteTarget', () => {
    it('deletes target', async () => {
      mockTargetsDelete.mockResolvedValue(undefined);
      await expect(service.deleteTarget('t-1')).resolves.not.toThrow();
      expect(mockTargetsDelete).toHaveBeenCalledWith('t-1');
    });
  });

  describe('probeTarget', () => {
    it('probes target connection', async () => {
      mockTargetsProbe.mockResolvedValue({ status: 'connected', latency_ms: 42 });
      const result = await service.probeTarget({ api_endpoint: 'https://example.com' });
      expect(result).toEqual({ status: 'connected', latency_ms: 42 });
      expect(mockTargetsProbe).toHaveBeenCalledWith({ api_endpoint: 'https://example.com' });
    });
  });

  describe('getTargetProfile', () => {
    it('returns target profile', async () => {
      mockTargetsGetProfile.mockResolvedValue({
        profiling_status: 'COMPLETED',
        categories: ['safety'],
      });
      const result = await service.getTargetProfile('t-1');
      expect(result).toEqual({ profiling_status: 'COMPLETED', categories: ['safety'] });
      expect(mockTargetsGetProfile).toHaveBeenCalledWith('t-1');
    });
  });

  describe('updateTargetProfile', () => {
    it('updates target profile', async () => {
      mockTargetsUpdateProfile.mockResolvedValue({ profiling_status: 'PENDING' });
      const result = await service.updateTargetProfile('t-1', { categories: ['security'] });
      expect(result).toEqual({ profiling_status: 'PENDING' });
      expect(mockTargetsUpdateProfile).toHaveBeenCalledWith('t-1', { categories: ['security'] });
    });
  });

  describe('waitForCompletion', () => {
    it('resolves when status is COMPLETED', async () => {
      mockScansGet
        .mockResolvedValueOnce({
          uuid: 'job-1',
          name: 'Scan',
          status: 'RUNNING',
          job_type: 'STATIC',
          target_id: 't-1',
          target: { name: 'T' },
          completed: 50,
          total: 100,
        })
        .mockResolvedValueOnce({
          uuid: 'job-1',
          name: 'Scan',
          status: 'COMPLETED',
          job_type: 'STATIC',
          target_id: 't-1',
          target: { name: 'T' },
          completed: 100,
          total: 100,
          score: 85,
        });

      const onProgress = vi.fn();
      const result = await service.waitForCompletion('job-1', onProgress, 10);
      expect(result.status).toBe('COMPLETED');
      expect(result.score).toBe(85);
      expect(onProgress).toHaveBeenCalledTimes(2);
    });

    it('resolves on PARTIALLY_COMPLETE', async () => {
      mockScansGet.mockResolvedValue({
        uuid: 'job-1',
        name: 'Scan',
        status: 'PARTIALLY_COMPLETE',
        job_type: 'STATIC',
        target_id: 't-1',
        target: { name: 'T' },
      });

      const result = await service.waitForCompletion('job-1', undefined, 10);
      expect(result.status).toBe('PARTIALLY_COMPLETE');
    });

    it('resolves on ABORTED', async () => {
      mockScansGet.mockResolvedValue({
        uuid: 'job-1',
        name: 'Scan',
        status: 'ABORTED',
        job_type: 'STATIC',
        target_id: 't-1',
        target: { name: 'T' },
      });

      const result = await service.waitForCompletion('job-1', undefined, 10);
      expect(result.status).toBe('ABORTED');
    });

    it('throws on FAILED', async () => {
      mockScansGet.mockResolvedValue({
        uuid: 'job-1',
        name: 'Scan',
        status: 'FAILED',
        job_type: 'STATIC',
        target_id: 't-1',
        target: { name: 'T' },
      });

      await expect(service.waitForCompletion('job-1', undefined, 10)).rejects.toThrow(
        'Scan job-1 failed',
      );
    });

    it('calls onProgress each poll cycle', async () => {
      mockScansGet
        .mockResolvedValueOnce({
          uuid: 'job-1',
          name: 'Scan',
          status: 'QUEUED',
          job_type: 'STATIC',
          target_id: 't-1',
          target: { name: 'T' },
        })
        .mockResolvedValueOnce({
          uuid: 'job-1',
          name: 'Scan',
          status: 'RUNNING',
          job_type: 'STATIC',
          target_id: 't-1',
          target: { name: 'T' },
          completed: 30,
          total: 100,
        })
        .mockResolvedValueOnce({
          uuid: 'job-1',
          name: 'Scan',
          status: 'COMPLETED',
          job_type: 'STATIC',
          target_id: 't-1',
          target: { name: 'T' },
          completed: 100,
          total: 100,
        });

      const onProgress = vi.fn();
      await service.waitForCompletion('job-1', onProgress, 10);
      expect(onProgress).toHaveBeenCalledTimes(3);
      expect(onProgress.mock.calls[0][0].status).toBe('QUEUED');
      expect(onProgress.mock.calls[1][0].status).toBe('RUNNING');
      expect(onProgress.mock.calls[2][0].status).toBe('COMPLETED');
    });
  });

  describe('getEulaContent', () => {
    it('returns EULA content', async () => {
      mockEulaGetContent.mockResolvedValue({ content: 'EULA text here' });
      const result = await service.getEulaContent();
      expect(result).toEqual({ content: 'EULA text here' });
      expect(mockEulaGetContent).toHaveBeenCalled();
    });
  });

  describe('getEulaStatus', () => {
    it('returns accepted status', async () => {
      mockEulaGetStatus.mockResolvedValue({
        uuid: 'eula-1',
        is_accepted: true,
        accepted_at: '2026-03-01T00:00:00Z',
        accepted_by_user_id: 'user-1',
      });
      const result = await service.getEulaStatus();
      expect(result).toEqual({
        isAccepted: true,
        acceptedAt: '2026-03-01T00:00:00Z',
        acceptedByUserId: 'user-1',
      });
    });

    it('returns not-accepted status', async () => {
      mockEulaGetStatus.mockResolvedValue({
        is_accepted: false,
      });
      const result = await service.getEulaStatus();
      expect(result).toEqual({
        isAccepted: false,
        acceptedAt: undefined,
        acceptedByUserId: undefined,
      });
    });
  });

  describe('acceptEula', () => {
    it('accepts EULA and returns status', async () => {
      mockEulaAccept.mockResolvedValue({
        uuid: 'eula-1',
        is_accepted: true,
        accepted_at: '2026-03-31T00:00:00Z',
        accepted_by_user_id: 'user-1',
      });
      const result = await service.acceptEula('EULA text here');
      expect(result).toEqual({
        isAccepted: true,
        acceptedAt: '2026-03-31T00:00:00Z',
        acceptedByUserId: 'user-1',
      });
      expect(mockEulaAccept).toHaveBeenCalledWith({
        eula_content: 'EULA text here',
        accepted_at: expect.any(String),
      });
    });
  });
});
