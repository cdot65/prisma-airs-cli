import { beforeEach, describe, expect, it, vi } from 'vitest';
import { SdkModelSecurityService } from '../../../src/airs/modelsecurity.js';

// -- Mock fns for ModelSecurityClient sub-clients --
const mockScansCreate = vi.fn();
const mockScansList = vi.fn();
const mockScansGet = vi.fn();
const mockScansGetEvaluations = vi.fn();
const mockScansGetEvaluation = vi.fn();
const mockScansGetViolations = vi.fn();
const mockScansGetViolation = vi.fn();
const mockScansGetFiles = vi.fn();
const mockScansAddLabels = vi.fn();
const mockScansSetLabels = vi.fn();
const mockScansDeleteLabels = vi.fn();
const mockScansGetLabelKeys = vi.fn();
const mockScansGetLabelValues = vi.fn();

const mockGroupsCreate = vi.fn();
const mockGroupsList = vi.fn();
const mockGroupsGet = vi.fn();
const mockGroupsUpdate = vi.fn();
const mockGroupsDelete = vi.fn();
const mockGroupsListRuleInstances = vi.fn();
const mockGroupsGetRuleInstance = vi.fn();
const mockGroupsUpdateRuleInstance = vi.fn();

const mockRulesList = vi.fn();
const mockRulesGet = vi.fn();

const mockGetPyPIAuth = vi.fn();

function makeMockClient() {
  return {
    scans: {
      create: mockScansCreate,
      list: mockScansList,
      get: mockScansGet,
      getEvaluations: mockScansGetEvaluations,
      getEvaluation: mockScansGetEvaluation,
      getViolations: mockScansGetViolations,
      getViolation: mockScansGetViolation,
      getFiles: mockScansGetFiles,
      addLabels: mockScansAddLabels,
      setLabels: mockScansSetLabels,
      deleteLabels: mockScansDeleteLabels,
      getLabelKeys: mockScansGetLabelKeys,
      getLabelValues: mockScansGetLabelValues,
    },
    securityGroups: {
      create: mockGroupsCreate,
      list: mockGroupsList,
      get: mockGroupsGet,
      update: mockGroupsUpdate,
      delete: mockGroupsDelete,
      listRuleInstances: mockGroupsListRuleInstances,
      getRuleInstance: mockGroupsGetRuleInstance,
      updateRuleInstance: mockGroupsUpdateRuleInstance,
    },
    securityRules: {
      list: mockRulesList,
      get: mockRulesGet,
    },
    getPyPIAuth: mockGetPyPIAuth,
  };
}

vi.mock('@cdot65/prisma-airs-sdk', () => ({
  ModelSecurityClient: vi.fn().mockImplementation(() => makeMockClient()),
}));

describe('SdkModelSecurityService', () => {
  let service: SdkModelSecurityService;

  beforeEach(() => {
    vi.clearAllMocks();
    service = new SdkModelSecurityService({
      clientId: 'test-id',
      clientSecret: 'test-secret',
      tsgId: 'tsg-123',
    });
  });

  // -----------------------------------------------------------------------
  // Security Groups
  // -----------------------------------------------------------------------
  describe('listGroups', () => {
    it('returns normalized group list', async () => {
      mockGroupsList.mockResolvedValue({
        pagination: { total_items: 2 },
        security_groups: [
          {
            uuid: 'g-1',
            name: 'Group 1',
            description: 'First group',
            source_type: 'HUGGING_FACE',
            state: 'ACTIVE',
            created_at: '2026-01-01T00:00:00Z',
            updated_at: '2026-01-02T00:00:00Z',
          },
          {
            uuid: 'g-2',
            name: 'Group 2',
            description: '',
            source_type: 'LOCAL',
            state: 'PENDING',
            created_at: '2026-01-03T00:00:00Z',
            updated_at: '2026-01-04T00:00:00Z',
          },
        ],
      });

      const result = await service.listGroups();
      expect(result.totalItems).toBe(2);
      expect(result.groups).toEqual([
        {
          uuid: 'g-1',
          name: 'Group 1',
          description: 'First group',
          sourceType: 'HUGGING_FACE',
          state: 'ACTIVE',
          createdAt: '2026-01-01T00:00:00Z',
          updatedAt: '2026-01-02T00:00:00Z',
        },
        {
          uuid: 'g-2',
          name: 'Group 2',
          description: '',
          sourceType: 'LOCAL',
          state: 'PENDING',
          createdAt: '2026-01-03T00:00:00Z',
          updatedAt: '2026-01-04T00:00:00Z',
        },
      ]);
    });

    it('passes filter options through', async () => {
      mockGroupsList.mockResolvedValue({
        pagination: { total_items: 0 },
        security_groups: [],
      });

      await service.listGroups({
        sourceTypes: ['HUGGING_FACE'],
        searchQuery: 'prod',
        sortField: 'created_at',
        sortDir: 'desc',
        enabledRules: ['r-1'],
        limit: 5,
      });

      expect(mockGroupsList).toHaveBeenCalledWith({
        source_types: ['HUGGING_FACE'],
        search_query: 'prod',
        sort_field: 'created_at',
        sort_dir: 'desc',
        enabled_rules: ['r-1'],
        limit: 5,
      });
    });
  });

  describe('getGroup', () => {
    it('returns normalized group detail', async () => {
      mockGroupsGet.mockResolvedValue({
        uuid: 'g-1',
        name: 'Group 1',
        description: 'desc',
        source_type: 'S3',
        state: 'ACTIVE',
        created_at: '2026-01-01T00:00:00Z',
        updated_at: '2026-01-02T00:00:00Z',
      });

      const result = await service.getGroup('g-1');
      expect(result.uuid).toBe('g-1');
      expect(result.sourceType).toBe('S3');
    });
  });

  describe('createGroup', () => {
    it('returns normalized created group', async () => {
      mockGroupsCreate.mockResolvedValue({
        uuid: 'g-new',
        name: 'New Group',
        description: 'desc',
        source_type: 'HUGGING_FACE',
        state: 'PENDING',
        created_at: '2026-01-01T00:00:00Z',
        updated_at: '2026-01-01T00:00:00Z',
      });

      const result = await service.createGroup({
        name: 'New Group',
        sourceType: 'HUGGING_FACE',
        description: 'desc',
      });

      expect(result.uuid).toBe('g-new');
      expect(mockGroupsCreate).toHaveBeenCalledWith({
        name: 'New Group',
        source_type: 'HUGGING_FACE',
        description: 'desc',
      });
    });

    it('passes rule configurations', async () => {
      mockGroupsCreate.mockResolvedValue({
        uuid: 'g-new',
        name: 'G',
        description: '',
        source_type: 'LOCAL',
        state: 'PENDING',
        created_at: '2026-01-01T00:00:00Z',
        updated_at: '2026-01-01T00:00:00Z',
      });

      await service.createGroup({
        name: 'G',
        sourceType: 'LOCAL',
        ruleConfigurations: { 'r-1': { state: 'BLOCKING' } },
      });

      expect(mockGroupsCreate).toHaveBeenCalledWith({
        name: 'G',
        source_type: 'LOCAL',
        rule_configurations: { 'r-1': { state: 'BLOCKING' } },
      });
    });
  });

  describe('updateGroup', () => {
    it('returns normalized updated group', async () => {
      mockGroupsUpdate.mockResolvedValue({
        uuid: 'g-1',
        name: 'Updated',
        description: 'new desc',
        source_type: 'LOCAL',
        state: 'ACTIVE',
        created_at: '2026-01-01T00:00:00Z',
        updated_at: '2026-01-05T00:00:00Z',
      });

      const result = await service.updateGroup('g-1', { name: 'Updated', description: 'new desc' });
      expect(result.name).toBe('Updated');
      expect(mockGroupsUpdate).toHaveBeenCalledWith('g-1', {
        name: 'Updated',
        description: 'new desc',
      });
    });
  });

  describe('deleteGroup', () => {
    it('calls SDK delete', async () => {
      mockGroupsDelete.mockResolvedValue(undefined);
      await service.deleteGroup('g-1');
      expect(mockGroupsDelete).toHaveBeenCalledWith('g-1');
    });
  });

  // -----------------------------------------------------------------------
  // Rule Instances
  // -----------------------------------------------------------------------
  describe('listRuleInstances', () => {
    it('returns normalized rule instance list', async () => {
      mockGroupsListRuleInstances.mockResolvedValue({
        pagination: { total_items: 1 },
        rule_instances: [
          {
            uuid: 'ri-1',
            security_group_uuid: 'g-1',
            security_rule_uuid: 'r-1',
            state: 'BLOCKING',
            created_at: '2026-01-01T00:00:00Z',
            updated_at: '2026-01-02T00:00:00Z',
            rule: { uuid: 'r-1', name: 'Rule 1' },
            field_values: { approved_formats: ['safetensors'] },
          },
        ],
      });

      const result = await service.listRuleInstances('g-1');
      expect(result.totalItems).toBe(1);
      expect(result.ruleInstances[0]).toEqual({
        uuid: 'ri-1',
        securityGroupUuid: 'g-1',
        securityRuleUuid: 'r-1',
        state: 'BLOCKING',
        createdAt: '2026-01-01T00:00:00Z',
        updatedAt: '2026-01-02T00:00:00Z',
        rule: { uuid: 'r-1', name: 'Rule 1' },
        fieldValues: { approved_formats: ['safetensors'] },
      });
    });

    it('passes filter options', async () => {
      mockGroupsListRuleInstances.mockResolvedValue({
        pagination: { total_items: 0 },
        rule_instances: [],
      });

      await service.listRuleInstances('g-1', {
        securityRuleUuid: 'r-1',
        state: 'BLOCKING',
      });

      expect(mockGroupsListRuleInstances).toHaveBeenCalledWith('g-1', {
        security_rule_uuid: 'r-1',
        state: 'BLOCKING',
      });
    });
  });

  describe('getRuleInstance', () => {
    it('returns normalized rule instance', async () => {
      mockGroupsGetRuleInstance.mockResolvedValue({
        uuid: 'ri-1',
        security_group_uuid: 'g-1',
        security_rule_uuid: 'r-1',
        state: 'ALLOWING',
        created_at: '2026-01-01T00:00:00Z',
        updated_at: '2026-01-02T00:00:00Z',
        rule: { uuid: 'r-1', name: 'Rule 1' },
        field_values: {},
      });

      const result = await service.getRuleInstance('g-1', 'ri-1');
      expect(result.uuid).toBe('ri-1');
      expect(result.state).toBe('ALLOWING');
    });
  });

  describe('updateRuleInstance', () => {
    it('passes request to SDK and returns normalized result', async () => {
      mockGroupsUpdateRuleInstance.mockResolvedValue({
        uuid: 'ri-1',
        security_group_uuid: 'g-1',
        security_rule_uuid: 'r-1',
        state: 'BLOCKING',
        created_at: '2026-01-01T00:00:00Z',
        updated_at: '2026-01-03T00:00:00Z',
        rule: { uuid: 'r-1', name: 'Rule 1' },
        field_values: { approved_formats: ['onnx'] },
      });

      const result = await service.updateRuleInstance('g-1', 'ri-1', {
        state: 'BLOCKING',
        fieldValues: { approved_formats: ['onnx'] },
      });

      expect(result.state).toBe('BLOCKING');
      expect(mockGroupsUpdateRuleInstance).toHaveBeenCalledWith('g-1', 'ri-1', {
        security_group_uuid: 'g-1',
        state: 'BLOCKING',
        field_values: { approved_formats: ['onnx'] },
      });
    });
  });

  // -----------------------------------------------------------------------
  // Security Rules
  // -----------------------------------------------------------------------
  describe('listRules', () => {
    it('returns normalized rule list', async () => {
      mockRulesList.mockResolvedValue({
        pagination: { total_items: 1 },
        rules: [
          {
            uuid: 'r-1',
            name: 'Approved Formats',
            description: 'Check model formats',
            rule_type: 'ARTIFACT',
            compatible_sources: ['HUGGING_FACE', 'LOCAL'],
            default_state: 'ALLOWING',
            remediation: {
              description: 'Fix format',
              steps: ['Step 1'],
              url: 'https://example.com',
            },
            editable_fields: [],
            constant_values: {},
            default_values: {},
          },
        ],
      });

      const result = await service.listRules();
      expect(result.totalItems).toBe(1);
      expect(result.rules[0]).toEqual({
        uuid: 'r-1',
        name: 'Approved Formats',
        description: 'Check model formats',
        ruleType: 'ARTIFACT',
        compatibleSources: ['HUGGING_FACE', 'LOCAL'],
        defaultState: 'ALLOWING',
        remediation: { description: 'Fix format', steps: ['Step 1'], url: 'https://example.com' },
        editableFields: [],
        constantValues: {},
        defaultValues: {},
      });
    });

    it('passes filter options', async () => {
      mockRulesList.mockResolvedValue({ pagination: { total_items: 0 }, rules: [] });

      await service.listRules({ sourceType: 'LOCAL', searchQuery: 'format' });

      expect(mockRulesList).toHaveBeenCalledWith({
        source_type: 'LOCAL',
        search_query: 'format',
      });
    });
  });

  describe('getRule', () => {
    it('returns normalized rule', async () => {
      mockRulesGet.mockResolvedValue({
        uuid: 'r-1',
        name: 'Rule',
        description: 'desc',
        rule_type: 'METADATA',
        compatible_sources: ['S3'],
        default_state: 'DISABLED',
        remediation: { description: '', steps: [], url: '' },
        editable_fields: [
          {
            attribute_name: 'approved_licenses',
            type: 'string',
            display_name: 'Approved Licenses',
            display_type: 'LIST',
          },
        ],
        constant_values: {},
        default_values: { approved_licenses: [] },
      });

      const result = await service.getRule('r-1');
      expect(result.ruleType).toBe('METADATA');
      expect(result.editableFields).toHaveLength(1);
      expect(result.editableFields[0].attributeName).toBe('approved_licenses');
    });
  });

  // -----------------------------------------------------------------------
  // Scans
  // -----------------------------------------------------------------------
  describe('listScans', () => {
    it('returns normalized scan list', async () => {
      mockScansList.mockResolvedValue({
        pagination: { total_items: 1 },
        scans: [
          {
            uuid: 's-1',
            eval_outcome: 'BLOCKED',
            model_uri: 'https://huggingface.co/test/model',
            scan_origin: 'HUGGING_FACE',
            source_type: 'HUGGING_FACE',
            security_group_name: 'Default HUGGING_FACE',
            eval_summary: { rules_failed: 1, rules_passed: 5, total_rules: 6 },
            created_at: '2026-01-01T00:00:00Z',
            updated_at: '2026-01-02T00:00:00Z',
            labels: [{ key: 'env', value: 'prod' }],
          },
        ],
      });

      const result = await service.listScans();
      expect(result.totalItems).toBe(1);
      expect(result.scans[0]).toEqual({
        uuid: 's-1',
        evalOutcome: 'BLOCKED',
        modelUri: 'https://huggingface.co/test/model',
        scanOrigin: 'HUGGING_FACE',
        sourceType: 'HUGGING_FACE',
        securityGroupName: 'Default HUGGING_FACE',
        evalSummary: { rulesFailed: 1, rulesPassed: 5, totalRules: 6 },
        createdAt: '2026-01-01T00:00:00Z',
        updatedAt: '2026-01-02T00:00:00Z',
        labels: [{ key: 'env', value: 'prod' }],
      });
    });

    it('passes filter options', async () => {
      mockScansList.mockResolvedValue({ pagination: { total_items: 0 }, scans: [] });

      await service.listScans({
        evalOutcome: 'BLOCKED',
        sourceType: 'HUGGING_FACE',
        scanOrigin: 'MODEL_SECURITY_SDK',
        limit: 10,
      });

      expect(mockScansList).toHaveBeenCalledWith({
        eval_outcome: 'BLOCKED',
        source_type: 'HUGGING_FACE',
        scan_origin: 'MODEL_SECURITY_SDK',
        limit: 10,
      });
    });
  });

  describe('getScan', () => {
    it('returns normalized scan', async () => {
      mockScansGet.mockResolvedValue({
        uuid: 's-1',
        eval_outcome: 'ALLOWED',
        model_uri: 'gs://bucket/model',
        scan_origin: 'GCS',
        source_type: 'GCS',
        security_group_name: 'Default GCS',
        eval_summary: { rules_failed: 0, rules_passed: 3, total_rules: 3 },
        created_at: '2026-01-01T00:00:00Z',
        updated_at: '2026-01-02T00:00:00Z',
        labels: [],
      });

      const result = await service.getScan('s-1');
      expect(result.uuid).toBe('s-1');
      expect(result.evalOutcome).toBe('ALLOWED');
      expect(result.evalSummary?.totalRules).toBe(3);
    });
  });

  describe('createScan', () => {
    it('passes request to SDK and returns normalized result', async () => {
      mockScansCreate.mockResolvedValue({
        uuid: 's-new',
        eval_outcome: 'PENDING',
        model_uri: '',
        scan_origin: 'LOCAL',
        source_type: 'LOCAL',
        security_group_name: 'Default LOCAL',
        eval_summary: null,
        created_at: '2026-01-01T00:00:00Z',
        updated_at: '2026-01-01T00:00:00Z',
        labels: [],
      });

      const result = await service.createScan({ someField: 'value' });
      expect(result.uuid).toBe('s-new');
      expect(mockScansCreate).toHaveBeenCalledWith({ someField: 'value' });
    });
  });

  // -----------------------------------------------------------------------
  // Evaluations
  // -----------------------------------------------------------------------
  describe('getEvaluations', () => {
    it('returns normalized evaluation list', async () => {
      mockScansGetEvaluations.mockResolvedValue({
        pagination: { total_items: 1 },
        evaluations: [
          {
            uuid: 'e-1',
            result: 'FAILED',
            violation_count: 2,
            rule_instance_uuid: 'ri-1',
            rule_name: 'Format Check',
            rule_description: 'Check file format',
            rule_instance_state: 'BLOCKING',
          },
        ],
      });

      const result = await service.getEvaluations('s-1');
      expect(result.totalItems).toBe(1);
      expect(result.evaluations[0]).toEqual({
        uuid: 'e-1',
        result: 'FAILED',
        violationCount: 2,
        ruleInstanceUuid: 'ri-1',
        ruleName: 'Format Check',
        ruleDescription: 'Check file format',
        ruleInstanceState: 'BLOCKING',
      });
    });
  });

  describe('getEvaluation', () => {
    it('returns normalized single evaluation', async () => {
      mockScansGetEvaluation.mockResolvedValue({
        uuid: 'e-1',
        result: 'PASSED',
        violation_count: 0,
        rule_instance_uuid: 'ri-1',
        rule_name: 'License Check',
        rule_description: 'Check license',
        rule_instance_state: 'BLOCKING',
      });

      const result = await service.getEvaluation('e-1');
      expect(result.result).toBe('PASSED');
    });
  });

  // -----------------------------------------------------------------------
  // Violations
  // -----------------------------------------------------------------------
  describe('getViolations', () => {
    it('returns normalized violation list', async () => {
      mockScansGetViolations.mockResolvedValue({
        pagination: { total_items: 1 },
        violations: [
          {
            uuid: 'v-1',
            description: 'Pickle exploit detected',
            threat: 'MALICIOUS_CODE',
            threat_description: 'Code injection threat',
            file: 'model.pkl',
            rule_name: 'Malicious Code',
            rule_description: 'Scans for malicious code',
            rule_instance_state: 'BLOCKING',
          },
        ],
      });

      const result = await service.getViolations('s-1');
      expect(result.totalItems).toBe(1);
      expect(result.violations[0]).toEqual({
        uuid: 'v-1',
        description: 'Pickle exploit detected',
        threat: 'MALICIOUS_CODE',
        threatDescription: 'Code injection threat',
        file: 'model.pkl',
        ruleName: 'Malicious Code',
        ruleDescription: 'Scans for malicious code',
        ruleInstanceState: 'BLOCKING',
      });
    });
  });

  describe('getViolation', () => {
    it('returns normalized single violation', async () => {
      mockScansGetViolation.mockResolvedValue({
        uuid: 'v-1',
        description: 'Unapproved format',
        threat: 'UNAPPROVED_FORMATS',
        threat_description: 'Unapproved file format',
        file: 'model.bin',
        rule_name: 'Format Check',
        rule_description: 'Check file format',
        rule_instance_state: 'BLOCKING',
      });

      const result = await service.getViolation('v-1');
      expect(result.uuid).toBe('v-1');
      expect(result.ruleName).toBe('Format Check');
      expect(result.threat).toBe('UNAPPROVED_FORMATS');
    });
  });

  // -----------------------------------------------------------------------
  // Files
  // -----------------------------------------------------------------------
  describe('getFiles', () => {
    it('returns normalized file list', async () => {
      mockScansGetFiles.mockResolvedValue({
        pagination: { total_items: 1 },
        files: [
          {
            uuid: 'f-1',
            path: 'model.safetensors',
            type: 'FILE',
            formats: ['safetensors'],
            result: 'SUCCESS',
          },
        ],
      });

      const result = await service.getFiles('s-1');
      expect(result.totalItems).toBe(1);
      expect(result.files[0]).toEqual({
        uuid: 'f-1',
        path: 'model.safetensors',
        type: 'FILE',
        formats: ['safetensors'],
        result: 'SUCCESS',
      });
    });

    it('passes filter options', async () => {
      mockScansGetFiles.mockResolvedValue({ pagination: { total_items: 0 }, files: [] });

      await service.getFiles('s-1', { type: 'FILE', result: 'FAILED' });

      expect(mockScansGetFiles).toHaveBeenCalledWith('s-1', { type: 'FILE', result: 'FAILED' });
    });
  });

  // -----------------------------------------------------------------------
  // Labels
  // -----------------------------------------------------------------------
  describe('addLabels', () => {
    it('calls SDK addLabels', async () => {
      mockScansAddLabels.mockResolvedValue({});
      await service.addLabels('s-1', [{ key: 'env', value: 'prod' }]);
      expect(mockScansAddLabels).toHaveBeenCalledWith('s-1', {
        labels: [{ key: 'env', value: 'prod' }],
      });
    });
  });

  describe('setLabels', () => {
    it('calls SDK setLabels', async () => {
      mockScansSetLabels.mockResolvedValue({});
      await service.setLabels('s-1', [{ key: 'env', value: 'staging' }]);
      expect(mockScansSetLabels).toHaveBeenCalledWith('s-1', {
        labels: [{ key: 'env', value: 'staging' }],
      });
    });
  });

  describe('deleteLabels', () => {
    it('calls SDK deleteLabels', async () => {
      mockScansDeleteLabels.mockResolvedValue(undefined);
      await service.deleteLabels('s-1', ['env', 'team']);
      expect(mockScansDeleteLabels).toHaveBeenCalledWith('s-1', ['env', 'team']);
    });
  });

  describe('getLabelKeys', () => {
    it('returns label keys', async () => {
      mockScansGetLabelKeys.mockResolvedValue({
        pagination: { total_items: 2 },
        keys: ['env', 'team'],
      });

      const result = await service.getLabelKeys();
      expect(result).toEqual({ totalItems: 2, keys: ['env', 'team'] });
    });
  });

  describe('getLabelValues', () => {
    it('returns label values for a key', async () => {
      mockScansGetLabelValues.mockResolvedValue({
        pagination: { total_items: 2 },
        values: ['prod', 'staging'],
      });

      const result = await service.getLabelValues('env');
      expect(result).toEqual({ totalItems: 2, values: ['prod', 'staging'] });
    });
  });

  // -----------------------------------------------------------------------
  // PyPI Auth
  // -----------------------------------------------------------------------
  describe('getPyPIAuth', () => {
    it('returns auth response', async () => {
      mockGetPyPIAuth.mockResolvedValue({
        url: 'https://pypi.example.com',
        expires_at: '2026-01-01T01:00:00Z',
      });

      const result = await service.getPyPIAuth();
      expect(result).toEqual({
        url: 'https://pypi.example.com',
        expiresAt: '2026-01-01T01:00:00Z',
      });
    });
  });
});
