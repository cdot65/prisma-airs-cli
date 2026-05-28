import { RedTeamClient, type RedTeamClientOptions } from '@cdot65/prisma-airs-sdk';
import type {
  MutationResponse,
  PromptDetail,
  PromptSetDetail,
  PromptSetService,
  PromptSetVersionInfo,
  PropertyValueList,
} from './types.js';

/** Normalize SDK prompt set response to PromptSetDetail. */
function normalizePromptSet(raw: Record<string, unknown>): PromptSetDetail {
  return {
    uuid: raw.uuid as string,
    name: raw.name as string,
    active: raw.active as boolean,
    archive: raw.archive as boolean,
    description: raw.description as string | undefined,
    createdAt: raw.created_at as string | undefined,
    updatedAt: raw.updated_at as string | undefined,
  };
}

/** Normalize SDK prompt response to PromptDetail. */
function normalizePrompt(raw: Record<string, unknown>): PromptDetail {
  return {
    uuid: raw.uuid as string,
    prompt: raw.prompt as string,
    goal: raw.goal as string | undefined,
    active: raw.active as boolean,
    promptSetId: raw.prompt_set_id as string,
  };
}

/**
 * Wraps the SDK's RedTeamClient.customAttacks to implement PromptSetService.
 * Creates and populates custom prompt sets for AI Red Team.
 */
export class SdkPromptSetService implements PromptSetService {
  private client: RedTeamClient;

  constructor(opts?: RedTeamClientOptions) {
    this.client = new RedTeamClient(opts);
  }

  async createPromptSet(
    name: string,
    description?: string,
  ): Promise<{ uuid: string; name: string }> {
    const response = await this.client.customAttacks.createPromptSet({
      name,
      ...(description ? { description } : {}),
    });
    return { uuid: response.uuid, name: response.name };
  }

  async addPrompt(
    promptSetId: string,
    prompt: string,
    goal?: string,
  ): Promise<{ uuid: string; prompt: string }> {
    const response = await this.client.customAttacks.createPrompt({
      prompt,
      prompt_set_id: promptSetId,
      ...(goal ? { goal } : {}),
    });
    return { uuid: response.uuid, prompt: response.prompt };
  }

  async listPromptSets(): Promise<Array<{ uuid: string; name: string; active: boolean }>> {
    const response = await this.client.customAttacks.listPromptSets();
    return (response.data ?? []).map((ps) => ({
      uuid: ps.uuid,
      name: ps.name,
      active: ps.active,
    }));
  }

  async getPromptSet(uuid: string): Promise<PromptSetDetail> {
    const response = await this.client.customAttacks.getPromptSet(uuid);
    return normalizePromptSet(response as unknown as Record<string, unknown>);
  }

  async updatePromptSet(
    uuid: string,
    request: { name?: string; description?: string },
  ): Promise<PromptSetDetail> {
    const response = await this.client.customAttacks.updatePromptSet(uuid, request as never);
    return normalizePromptSet(response as unknown as Record<string, unknown>);
  }

  async archivePromptSet(uuid: string, archive: boolean): Promise<void> {
    await this.client.customAttacks.archivePromptSet(uuid, { archive } as never);
  }

  async getPromptSetVersionInfo(uuid: string): Promise<PromptSetVersionInfo> {
    const response = await this.client.customAttacks.getPromptSetVersionInfo(uuid);
    const raw = response as unknown as Record<string, unknown>;
    return {
      uuid: raw.uuid as string,
      version: raw.version as number,
      stats: raw.stats as PromptSetVersionInfo['stats'],
    };
  }

  async downloadTemplate(uuid: string): Promise<string> {
    return this.client.customAttacks.downloadTemplate(uuid);
  }

  async uploadPromptsCsv(uuid: string, file: Blob): Promise<{ message: string; status: number }> {
    const response = await this.client.customAttacks.uploadPromptsCsv(uuid, file);
    return response as unknown as { message: string; status: number };
  }

  async listPrompts(
    setUuid: string,
    opts?: { limit?: number; skip?: number },
  ): Promise<PromptDetail[]> {
    const response = await this.client.customAttacks.listPrompts(setUuid, opts);
    return (
      (response as unknown as Record<string, unknown>).data as Array<Record<string, unknown>>
    ).map(normalizePrompt);
  }

  async getPrompt(setUuid: string, promptUuid: string): Promise<PromptDetail> {
    const response = await this.client.customAttacks.getPrompt(setUuid, promptUuid);
    return normalizePrompt(response as unknown as Record<string, unknown>);
  }

  async updatePrompt(
    setUuid: string,
    promptUuid: string,
    request: { prompt?: string; goal?: string },
  ): Promise<PromptDetail> {
    const response = await this.client.customAttacks.updatePrompt(
      setUuid,
      promptUuid,
      request as never,
    );
    return normalizePrompt(response as unknown as Record<string, unknown>);
  }

  async deletePrompt(setUuid: string, promptUuid: string): Promise<void> {
    await this.client.customAttacks.deletePrompt(setUuid, promptUuid);
  }

  async getPropertyNames(): Promise<string[]> {
    const response = await this.client.customAttacks.getPropertyNames();
    const raw = response as unknown as { data?: string[] };
    return raw.data ?? [];
  }

  async createPropertyName(name: string): Promise<MutationResponse> {
    const response = await this.client.customAttacks.createPropertyName({ name } as never);
    return response as unknown as MutationResponse;
  }

  async getPropertyValues(name: string): Promise<PropertyValueList> {
    const response = await this.client.customAttacks.getPropertyValues(name);
    const raw = response as unknown as { name?: string; values?: string[] };
    return { name: raw.name ?? name, values: raw.values ?? [] };
  }

  async createPropertyValue(name: string, value: string): Promise<MutationResponse> {
    const response = await this.client.customAttacks.createPropertyValue({
      property_name: name,
      property_value: value,
    });
    return response as unknown as MutationResponse;
  }
}
