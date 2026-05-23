import type {
  DataPatternListParams,
  DataPatternPatchRequest,
  DataPatternRequest,
  DataPatternResponse,
  ManagementClientOptions,
  PageDataPatternResponse,
} from '@cdot65/prisma-airs-sdk';
import { getOrCreateManagementClient } from '../management.js';
import type { DataPatternsService } from './types.js';

export class SdkDataPatternsService implements DataPatternsService {
  private readonly client;

  constructor(opts?: ManagementClientOptions) {
    this.client = getOrCreateManagementClient(opts).dlp.dataPatterns;
  }

  async list(params?: DataPatternListParams): Promise<PageDataPatternResponse> {
    return this.client.list(params);
  }

  async create(body: DataPatternRequest): Promise<DataPatternResponse> {
    return this.client.create(body);
  }

  async get(id: string): Promise<DataPatternResponse> {
    return this.client.get(id);
  }

  async replace(id: string, body: DataPatternRequest): Promise<DataPatternResponse> {
    return this.client.replace(id, body);
  }

  async patch(id: string, body: DataPatternPatchRequest): Promise<DataPatternResponse> {
    return this.client.patch(id, body);
  }

  async delete(id: string): Promise<void> {
    await this.client.delete(id);
  }
}
