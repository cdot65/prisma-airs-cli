import type {
  DictionaryGetParams,
  DictionaryListParams,
  DictionaryPatchRequest,
  DictionaryResponse,
  DictionaryUploadParams,
  ManagementClientOptions,
  PageDictionaryResponse,
} from '@cdot65/prisma-airs-sdk';
import { getOrCreateManagementClient } from '../management.js';
import type { DictionariesService, DictionaryReplaceFallback } from './types.js';

export class SdkDictionariesService implements DictionariesService {
  private readonly client;

  constructor(opts?: ManagementClientOptions) {
    this.client = getOrCreateManagementClient(opts).dlp.dictionaries;
  }

  async list(params?: DictionaryListParams): Promise<PageDictionaryResponse> {
    return this.client.list(params);
  }

  async create(params: DictionaryUploadParams): Promise<DictionaryResponse> {
    return this.client.create(params);
  }

  async get(id: string, params?: DictionaryGetParams): Promise<DictionaryResponse> {
    return this.client.get(id, params);
  }

  async replace(
    id: string,
    params: DictionaryUploadParams,
  ): Promise<DictionaryResponse | DictionaryReplaceFallback> {
    const r = await this.client.replace(id, params);
    if (r !== undefined) return r;
    try {
      return await this.client.get(id);
    } catch {
      return { kind: 'fallback', id };
    }
  }

  async patch(id: string, body: DictionaryPatchRequest): Promise<DictionaryResponse> {
    return this.client.patch(id, body);
  }

  async delete(id: string): Promise<void> {
    await this.client.delete(id);
  }
}
