import type {
  AdvancedDataProfileRequest,
  DataProfileListParams,
  DataProfilePatchRequest,
  DataProfileResponse,
  ManagementClientOptions,
  PageDataProfileResponse,
} from '@cdot65/prisma-airs-sdk';
import { getOrCreateManagementClient } from '../management.js';
import type { DataProfilesService } from './types.js';

export class SdkDataProfilesService implements DataProfilesService {
  private readonly client;

  constructor(opts?: ManagementClientOptions) {
    this.client = getOrCreateManagementClient(opts).dlp.dataProfiles;
  }

  async list(params?: DataProfileListParams): Promise<PageDataProfileResponse> {
    return this.client.list(params);
  }

  async create(body: AdvancedDataProfileRequest): Promise<DataProfileResponse> {
    return this.client.create(body);
  }

  async get(id: string): Promise<DataProfileResponse> {
    return this.client.get(id);
  }

  async replace(id: string, body: AdvancedDataProfileRequest): Promise<DataProfileResponse> {
    return this.client.replace(id, body);
  }

  async patch(id: string, body: DataProfilePatchRequest): Promise<DataProfileResponse> {
    return this.client.patch(id, body);
  }
}
