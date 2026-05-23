import type {
  DataFilteringProfileListParams,
  DataFilteringProfileRequest,
  DataFilteringProfileResponse,
  ManagementClientOptions,
  PageDataFilteringProfileResponse,
} from '@cdot65/prisma-airs-sdk';
import { getOrCreateManagementClient } from '../management.js';
import type { DataFilteringProfilesService } from './types.js';

export class SdkDataFilteringProfilesService implements DataFilteringProfilesService {
  private readonly client;

  constructor(opts?: ManagementClientOptions) {
    this.client = getOrCreateManagementClient(opts).dlp.dataFilteringProfiles;
  }

  async list(params?: DataFilteringProfileListParams): Promise<PageDataFilteringProfileResponse> {
    return this.client.list(params);
  }

  async get(id: string): Promise<DataFilteringProfileResponse> {
    return this.client.get(id);
  }

  async replace(
    id: string,
    body: DataFilteringProfileRequest,
  ): Promise<DataFilteringProfileResponse> {
    return this.client.replace(id, body);
  }
}
