import { beforeEach, describe, expect, it, vi } from 'vitest';
import { preserveVariablesForUpdate, SdkRedTeamService } from '../../../src/airs/redteam.js';

const mockAdaptersList = vi.fn();
const mockAdaptersGet = vi.fn();
const mockAdaptersCreate = vi.fn();
const mockAdaptersUpdate = vi.fn();
const mockAdaptersDelete = vi.fn();
const mockAdaptersValidate = vi.fn();

vi.mock('@cdot65/prisma-airs-sdk', () => ({
  RedTeamClient: vi.fn().mockImplementation(() => ({
    adapters: {
      list: mockAdaptersList,
      get: mockAdaptersGet,
      create: mockAdaptersCreate,
      update: mockAdaptersUpdate,
      delete: mockAdaptersDelete,
      validate: mockAdaptersValidate,
    },
  })),
}));

const listItem = {
  uuid: 'ad-uuid-1',
  name: 'my-adapter',
  status: 'ACTIVE',
  created_at: '2026-08-01T00:00:00Z',
  updated_at: '2026-08-01T01:00:00Z',
  created_by_user_id: 'user-1',
  target_count: 2,
};

const detail = {
  uuid: 'ad-uuid-1',
  tsg_id: 'tsg-1',
  name: 'my-adapter',
  script_b64: 'c2NyaXB0',
  status: 'ACTIVE',
  description: 'desc',
  network_broker_channel_uuid: 'ch-uuid-1',
  variables: [
    { key: 'endpoint', value: 'http://agent.svc:8080', type: 'VAR' },
    { key: 'client_secret', value: '**********', type: 'SECRET', is_redacted: true },
  ],
  target_count: 2,
  created_at: '2026-08-01T00:00:00Z',
  updated_at: '2026-08-01T01:00:00Z',
};

describe('SdkRedTeamService adapters', () => {
  let service: SdkRedTeamService;

  beforeEach(() => {
    vi.clearAllMocks();
    service = new SdkRedTeamService();
  });

  describe('listAdapters', () => {
    it('normalizes rows and maps offset to skip', async () => {
      mockAdaptersList.mockResolvedValue({ pagination: { total_items: 1 }, data: [listItem] });
      const result = await service.listAdapters({ limit: 10, offset: 5, search: 'my' });
      expect(mockAdaptersList).toHaveBeenCalledWith({ limit: 10, skip: 5, search: 'my' });
      expect(result.totalItems).toBe(1);
      expect(result.adapters).toEqual([
        {
          uuid: 'ad-uuid-1',
          name: 'my-adapter',
          status: 'ACTIVE',
          createdAt: '2026-08-01T00:00:00Z',
          updatedAt: '2026-08-01T01:00:00Z',
          createdByUserId: 'user-1',
          targetCount: 2,
        },
      ]);
    });
  });

  describe('getAdapter', () => {
    it('normalizes the detail including redaction flags', async () => {
      mockAdaptersGet.mockResolvedValue(detail);
      const adapter = await service.getAdapter('ad-uuid-1');
      expect(adapter.scriptB64).toBe('c2NyaXB0');
      expect(adapter.networkBrokerChannelUuid).toBe('ch-uuid-1');
      expect(adapter.variables).toEqual([
        { key: 'endpoint', value: 'http://agent.svc:8080', type: 'VAR', isRedacted: undefined },
        { key: 'client_secret', value: '**********', type: 'SECRET', isRedacted: true },
      ]);
    });
  });

  describe('createAdapter', () => {
    it('maps camelCase request to snake_case and passes validate option', async () => {
      mockAdaptersCreate.mockResolvedValue(detail);
      await service.createAdapter(
        {
          name: 'my-adapter',
          scriptB64: 'c2NyaXB0',
          prompt: 'Hello',
          networkBrokerChannelUuid: 'ch-uuid-1',
          variables: [{ key: 'endpoint', value: 'http://x', type: 'VAR' }],
        },
        false,
      );
      expect(mockAdaptersCreate).toHaveBeenCalledWith(
        {
          name: 'my-adapter',
          script_b64: 'c2NyaXB0',
          prompt: 'Hello',
          network_broker_channel_uuid: 'ch-uuid-1',
          variables: [{ key: 'endpoint', value: 'http://x', type: 'VAR' }],
        },
        { validate: false },
      );
    });
  });

  describe('updateAdapter (read-modify-write)', () => {
    it('merges overrides onto the current record so variables are never silently wiped', async () => {
      mockAdaptersGet.mockResolvedValue(detail);
      mockAdaptersUpdate.mockResolvedValue(detail);
      await service.updateAdapter('ad-uuid-1', { description: 'new desc', prompt: 'Hi' });
      expect(mockAdaptersUpdate).toHaveBeenCalledWith(
        'ad-uuid-1',
        {
          name: 'my-adapter',
          script_b64: 'c2NyaXB0',
          prompt: 'Hi',
          description: 'new desc',
          network_broker_channel_uuid: 'ch-uuid-1',
          variables: [
            { key: 'endpoint', value: 'http://agent.svc:8080', type: 'VAR' },
            // redacted secret resent as null => "keep stored value"
            { key: 'client_secret', value: null, type: 'SECRET' },
          ],
        },
        undefined,
      );
    });

    it('lets an explicit variables array replace the whole set', async () => {
      mockAdaptersGet.mockResolvedValue(detail);
      mockAdaptersUpdate.mockResolvedValue(detail);
      await service.updateAdapter('ad-uuid-1', {
        prompt: 'Hi',
        variables: [{ key: 'only', value: 'v', type: 'VAR' }],
      });
      const body = mockAdaptersUpdate.mock.calls[0][1];
      expect(body.variables).toEqual([{ key: 'only', value: 'v', type: 'VAR' }]);
    });
  });

  describe('deleteAdapter', () => {
    it('calls delete with the uuid', async () => {
      mockAdaptersDelete.mockResolvedValue(undefined);
      await service.deleteAdapter('ad-uuid-1');
      expect(mockAdaptersDelete).toHaveBeenCalledWith('ad-uuid-1');
    });
  });

  describe('validateAdapter', () => {
    it('passes the request through in snake_case and normalizes the outcome', async () => {
      mockAdaptersValidate.mockResolvedValue({
        validated: false,
        stdout: 'out',
        stderr: 'boom',
        traceback: 'tb',
      });
      const result = await service.validateAdapter({
        scriptB64: 'c2NyaXB0',
        networkBrokerChannelUuid: 'ch-uuid-1',
        prompt: 'Hello',
        variables: [{ key: 'endpoint', value: 'http://x', type: 'VAR' }],
      });
      expect(mockAdaptersValidate).toHaveBeenCalledWith({
        script_b64: 'c2NyaXB0',
        network_broker_channel_uuid: 'ch-uuid-1',
        prompt: 'Hello',
        variables: [{ key: 'endpoint', value: 'http://x', type: 'VAR' }],
      });
      expect(result).toEqual({ validated: false, stdout: 'out', stderr: 'boom', traceback: 'tb' });
    });

    it('auto-fills the full variables array from the adapter when adapterUuid is given without variables', async () => {
      mockAdaptersGet.mockResolvedValue(detail);
      mockAdaptersValidate.mockResolvedValue({ validated: true });
      await service.validateAdapter({
        scriptB64: 'c2NyaXB0',
        networkBrokerChannelUuid: 'ch-uuid-1',
        prompt: 'Hello',
        adapterUuid: 'ad-uuid-1',
      });
      const body = mockAdaptersValidate.mock.calls[0][0];
      expect(body.adapter_uuid).toBe('ad-uuid-1');
      // full key set sent; redacted secret as null so it resolves from storage
      expect(body.variables).toEqual([
        { key: 'endpoint', value: 'http://agent.svc:8080', type: 'VAR' },
        { key: 'client_secret', value: null, type: 'SECRET' },
      ]);
    });
  });
});

describe('preserveVariablesForUpdate', () => {
  it('maps redacted variables to null values and keeps plain values', () => {
    expect(
      preserveVariablesForUpdate([
        { key: 'endpoint', value: 'http://x', type: 'VAR' },
        { key: 'secret', value: '**********', type: 'SECRET', isRedacted: true },
        { key: 'empty', type: 'VAR' },
      ]),
    ).toEqual([
      { key: 'endpoint', value: 'http://x', type: 'VAR' },
      { key: 'secret', value: null, type: 'SECRET' },
      { key: 'empty', value: null, type: 'VAR' },
    ]);
  });
});
