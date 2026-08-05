import {
  AIGatewayClient,
  type AIGatewayClientOptions,
  type GatewayWorkspaceCreateRequest,
  type GatewayWorkspaceUpdateRequest,
} from '@cdot65/prisma-airs-sdk';
import type {
  AiGatewayService,
  AiGatewayWorkspace,
  AiGatewayWorkspaceCreateRequest,
  AiGatewayWorkspaceDetail,
  AiGatewayWorkspaceGetOptions,
  AiGatewayWorkspaceListOptions,
  AiGatewayWorkspaceUpdateRequest,
} from './types.js';

/**
 * `usage_limits`/`rate_limits` are `array | record | null` on the wire — the
 * array of policy objects is canonical, but the legacy single-object form is
 * still accepted upstream. Normalize everything to an array.
 */
function toLimitArray(value: unknown): Array<Record<string, unknown>> {
  if (Array.isArray(value)) return value as Array<Record<string, unknown>>;
  if (value !== null && typeof value === 'object') return [value as Record<string, unknown>];
  return [];
}

/** Normalize an SDK workspace list row into an AiGatewayWorkspace. */
function normalizeWorkspace(raw: Record<string, unknown>): AiGatewayWorkspace {
  return {
    id: raw.id as string,
    slug: raw.slug as string,
    name: raw.name as string,
    icon: raw.icon as string | null | undefined,
    description: raw.description as string | null | undefined,
    createdAt: raw.created_at as string | undefined,
    lastUpdatedAt: raw.last_updated_at as string | undefined,
    isDefault: Boolean(raw.is_default),
    status: raw.status as string | null | undefined,
    scopeName: raw.scope_name as string | undefined,
  };
}

/** Normalize an SDK workspace detail into an AiGatewayWorkspaceDetail. */
function normalizeWorkspaceDetail(raw: Record<string, unknown>): AiGatewayWorkspaceDetail {
  return {
    ...normalizeWorkspace(raw),
    defaults: raw.defaults as Record<string, unknown> | null | undefined,
    usageLimits: toLimitArray(raw.usage_limits),
    rateLimits: toLimitArray(raw.rate_limits),
    securitySettings: raw.security_settings as Record<string, boolean> | undefined,
    dataPlaneSecuritySettings: raw.data_plane_security_settings as
      | Record<string, unknown>
      | undefined,
    settings: raw.settings as Record<string, unknown> | undefined,
  };
}

/**
 * A 403 from the AI Gateway is a grant problem, and which grant depends on the
 * plane: `errorCode AB03` means the workspace-scope grant is missing (data
 * plane); otherwise the tenant-root admin grant is (admin plane). Returns a
 * user-facing hint, or undefined for non-403 errors.
 */
export function aiGatewayGrantHint(err: unknown): string | undefined {
  const status =
    (err as { status?: number; statusCode?: number })?.status ??
    (err as { statusCode?: number })?.statusCode;
  if (status !== 403) return undefined;
  const message = err instanceof Error ? err.message : String(err);
  const grant = message.includes('AB03')
    ? 'the service account is missing a workspace-scope grant (data plane, /ai_gw/v2)'
    : 'the service account is missing a tenant-root admin grant (admin plane, /ai_gw/admin/v2)';
  return (
    `${grant}. SCM Access Management edits the existing role row by default — ` +
    `use "Add Role" so the account ends up with both role rows, not one row moved.`
  );
}

/** Service wrapper over the SDK AIGatewayClient. */
export class SdkAiGatewayService implements AiGatewayService {
  private client: AIGatewayClient;

  constructor(opts?: AIGatewayClientOptions) {
    this.client = new AIGatewayClient(opts);
  }

  async listWorkspaces(options?: AiGatewayWorkspaceListOptions): Promise<AiGatewayWorkspace[]> {
    const response = await this.client.workspaces.list(options);
    return (response.data as Array<Record<string, unknown>>).map(normalizeWorkspace);
  }

  async listAllWorkspaces(): Promise<AiGatewayWorkspace[]> {
    // No single call returns both states: the API filters to active unless
    // asked otherwise, and only the admin plane sees the whole tenant.
    const [active, archived] = await Promise.all([
      this.client.workspaces.list({ plane: 'admin' }),
      this.client.workspaces.list({ plane: 'admin', status: 'archived' }),
    ]);
    return [
      ...(active.data as Array<Record<string, unknown>>),
      ...(archived.data as Array<Record<string, unknown>>),
    ].map(normalizeWorkspace);
  }

  async getWorkspace(
    workspaceRef: string,
    options?: AiGatewayWorkspaceGetOptions,
  ): Promise<AiGatewayWorkspaceDetail> {
    const raw = (await this.client.workspaces.get(workspaceRef, options)) as Record<
      string,
      unknown
    >;
    return normalizeWorkspaceDetail(raw);
  }

  async createWorkspace(
    request: AiGatewayWorkspaceCreateRequest,
  ): Promise<AiGatewayWorkspaceDetail> {
    const body: GatewayWorkspaceCreateRequest = {
      name: request.name,
      scope_name: request.scopeName,
    };
    if (request.description !== undefined) body.description = request.description;
    if (request.icon !== undefined) body.icon = request.icon;
    if (request.defaults !== undefined) body.defaults = request.defaults;
    if (request.users !== undefined) body.users = request.users;
    if (request.usageLimits !== undefined) body.usage_limits = request.usageLimits;
    if (request.rateLimits !== undefined) body.rate_limits = request.rateLimits;

    const created = (await this.client.workspaces.create(body)) as Record<string, unknown>;
    // create omits status, is_default, icon, both limit fields, and the
    // settings blocks — re-read for the full record. Admin plane, because a
    // fresh workspace's scope may not be granted to this service account yet.
    return this.refetchAfterWrite(created.id as string, created);
  }

  async updateWorkspace(
    workspaceRef: string,
    request: AiGatewayWorkspaceUpdateRequest,
  ): Promise<AiGatewayWorkspaceDetail> {
    const body: GatewayWorkspaceUpdateRequest = {};
    if (request.name !== undefined) body.name = request.name;
    if (request.description !== undefined) body.description = request.description;
    if (request.icon !== undefined) body.icon = request.icon;
    if (request.defaults !== undefined) body.defaults = request.defaults;
    if (request.usageLimits !== undefined) body.usage_limits = request.usageLimits;
    if (request.rateLimits !== undefined) body.rate_limits = request.rateLimits;

    await this.client.workspaces.update(workspaceRef, body);
    // update returns a literal `{}` — the write lands; re-read to display anything.
    return this.getWorkspace(workspaceRef, { plane: 'admin' });
  }

  async deleteWorkspace(workspaceRef: string): Promise<void> {
    // Soft delete. Deliberately no verify-by-get: an archived workspace
    // answers 404 AB08 on both planes even though list --status archived
    // still shows it.
    await this.client.workspaces.delete(workspaceRef);
  }

  /** Re-read after a write, falling back to the (partial) write response if the get fails. */
  private async refetchAfterWrite(
    workspaceRef: string,
    writeResponse: Record<string, unknown>,
  ): Promise<AiGatewayWorkspaceDetail> {
    try {
      return await this.getWorkspace(workspaceRef, { plane: 'admin' });
    } catch {
      return normalizeWorkspaceDetail(writeResponse);
    }
  }
}
