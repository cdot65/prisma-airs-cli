export type {
  AdvancedDataProfileRequest,
  DataFilteringProfileListParams,
  DataFilteringProfileRequest,
  DataFilteringProfileResponse,
  DataPatternListParams,
  DataPatternPatchRequest,
  DataPatternRequest,
  DataPatternResponse,
  DataProfileListParams,
  DataProfilePatchRequest,
  DataProfileResponse,
  DetectionRule,
  DictionaryFileInput,
  DictionaryGetParams,
  DictionaryListParams,
  DictionaryPatchRequest,
  DictionaryRequest,
  DictionaryResponse,
  DictionaryUploadParams,
  PageDataFilteringProfileResponse,
  PageDataPatternResponse,
  PageDataProfileResponse,
  PageDictionaryResponse,
} from '@cdot65/prisma-airs-sdk';

export interface DataFilteringProfilesService {
  list(
    params?: import('@cdot65/prisma-airs-sdk').DataFilteringProfileListParams,
  ): Promise<import('@cdot65/prisma-airs-sdk').PageDataFilteringProfileResponse>;
  get(id: string): Promise<import('@cdot65/prisma-airs-sdk').DataFilteringProfileResponse>;
  replace(
    id: string,
    body: import('@cdot65/prisma-airs-sdk').DataFilteringProfileRequest,
  ): Promise<import('@cdot65/prisma-airs-sdk').DataFilteringProfileResponse>;
}

export interface DataPatternsService {
  list(
    params?: import('@cdot65/prisma-airs-sdk').DataPatternListParams,
  ): Promise<import('@cdot65/prisma-airs-sdk').PageDataPatternResponse>;
  create(
    body: import('@cdot65/prisma-airs-sdk').DataPatternRequest,
  ): Promise<import('@cdot65/prisma-airs-sdk').DataPatternResponse>;
  get(id: string): Promise<import('@cdot65/prisma-airs-sdk').DataPatternResponse>;
  replace(
    id: string,
    body: import('@cdot65/prisma-airs-sdk').DataPatternRequest,
  ): Promise<import('@cdot65/prisma-airs-sdk').DataPatternResponse>;
  patch(
    id: string,
    body: import('@cdot65/prisma-airs-sdk').DataPatternPatchRequest,
  ): Promise<import('@cdot65/prisma-airs-sdk').DataPatternResponse>;
  delete(id: string): Promise<void>;
}

export interface DataProfilesService {
  list(
    params?: import('@cdot65/prisma-airs-sdk').DataProfileListParams,
  ): Promise<import('@cdot65/prisma-airs-sdk').PageDataProfileResponse>;
  create(
    body: import('@cdot65/prisma-airs-sdk').AdvancedDataProfileRequest,
  ): Promise<import('@cdot65/prisma-airs-sdk').DataProfileResponse>;
  get(id: string): Promise<import('@cdot65/prisma-airs-sdk').DataProfileResponse>;
  replace(
    id: string,
    body: import('@cdot65/prisma-airs-sdk').AdvancedDataProfileRequest,
  ): Promise<import('@cdot65/prisma-airs-sdk').DataProfileResponse>;
  patch(
    id: string,
    body: import('@cdot65/prisma-airs-sdk').DataProfilePatchRequest,
  ): Promise<import('@cdot65/prisma-airs-sdk').DataProfileResponse>;
}

/** Sentinel result for replace() when API returns 204 No Content. */
export interface DictionaryReplaceFallback {
  kind: 'fallback';
  id: string;
}

export interface DictionariesService {
  list(
    params?: import('@cdot65/prisma-airs-sdk').DictionaryListParams,
  ): Promise<import('@cdot65/prisma-airs-sdk').PageDictionaryResponse>;
  create(
    params: import('@cdot65/prisma-airs-sdk').DictionaryUploadParams,
  ): Promise<import('@cdot65/prisma-airs-sdk').DictionaryResponse>;
  get(
    id: string,
    params?: import('@cdot65/prisma-airs-sdk').DictionaryGetParams,
  ): Promise<import('@cdot65/prisma-airs-sdk').DictionaryResponse>;
  /** Returns the response on 200, re-gets on 204; returns fallback sentinel if re-get fails. */
  replace(
    id: string,
    params: import('@cdot65/prisma-airs-sdk').DictionaryUploadParams,
  ): Promise<import('@cdot65/prisma-airs-sdk').DictionaryResponse | DictionaryReplaceFallback>;
  patch(
    id: string,
    body: import('@cdot65/prisma-airs-sdk').DictionaryPatchRequest,
  ): Promise<import('@cdot65/prisma-airs-sdk').DictionaryResponse>;
  delete(id: string): Promise<void>;
}
