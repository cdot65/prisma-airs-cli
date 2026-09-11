import {
  AIGatewayChartFiltersSchema,
  AIGatewayClient,
  type AIGatewayClientOptions,
  type GatewayWorkspaceProvisionRequest,
  type GatewayWorkspaceUpdateRequest,
  type IamScope,
} from '@cdot65/prisma-airs-sdk';
import type {
  AiGatewayCostOptions,
  AiGatewayCostReport,
  AiGatewayPlane,
  AiGatewayScope,
  AiGatewayScopeCreateRequest,
  AiGatewayService,
  AiGatewayWorkspace,
  AiGatewayWorkspaceCreateRequest,
  AiGatewayWorkspaceDetail,
  AiGatewayWorkspaceGetOptions,
  AiGatewayWorkspaceListOptions,
  AiGatewayWorkspaceProvisionResult,
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

/** Normalize an SDK IAM scope into an AiGatewayScope. */
function normalizeScope(raw: IamScope): AiGatewayScope {
  return {
    name: raw.name,
    description: raw.description,
    resources: raw.resources.map((r) => ({
      resourceType: r.resource_type,
      resourceId: r.resource_id,
    })),
    tsgId: raw.tsg_id,
    id: raw.id,
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
    try {
      const raw = (await this.client.workspaces.get(workspaceRef, options)) as Record<
        string,
        unknown
      >;
      return normalizeWorkspaceDetail(raw);
    } catch (err) {
      // A display name 404s — resolve it against the list and retry once.
      const status = (err as { statusCode?: number }).statusCode;
      if (status !== 404) throw err;
      const resolved = await this.resolveWorkspaceRef(workspaceRef, [
        options?.plane ?? 'data',
        'admin',
      ]);
      if (resolved === workspaceRef) throw err;
      const raw = (await this.client.workspaces.get(resolved, options)) as Record<string, unknown>;
      return normalizeWorkspaceDetail(raw);
    }
  }

  async createWorkspace(
    request: AiGatewayWorkspaceCreateRequest,
  ): Promise<AiGatewayWorkspaceProvisionResult> {
    const body: GatewayWorkspaceProvisionRequest = { name: request.name };
    if (request.scopeName !== undefined) body.scope_name = request.scopeName;
    if (request.description !== undefined) body.description = request.description;
    if (request.icon !== undefined) body.icon = request.icon;
    if (request.defaults !== undefined) body.defaults = request.defaults;
    if (request.users !== undefined) body.users = request.users;
    if (request.usageLimits !== undefined) body.usage_limits = request.usageLimits;
    if (request.rateLimits !== undefined) body.rate_limits = request.rateLimits;

    // SCM's own order: IAM scope → workspace → PUT the scope back with the
    // workspace slug bound. A bare workspaces.create() against a scope that
    // does not exist yet is what produced the 400 AB01 seen on 2026-09-06.
    const result = await this.client.workspaces.provision(
      body,
      request.existingScope ? { existingScope: true } : {},
    );
    const created = result.workspace as unknown as Record<string, unknown>;
    // create omits status, is_default, icon, both limit fields, and the
    // settings blocks — re-read for the full record. Admin plane, because the
    // fresh scope is not granted to this service account.
    const workspace = await this.refetchAfterWrite(created.id as string, created);
    return { workspace, scope: normalizeScope(result.scope), scopeCreated: result.scopeCreated };
  }

  async listScopes(): Promise<AiGatewayScope[]> {
    const response = await this.client.iamScopes.list();
    return response.items.map(normalizeScope);
  }

  async getScope(name: string): Promise<AiGatewayScope> {
    return normalizeScope(await this.client.iamScopes.get(name));
  }

  async createScope(request: AiGatewayScopeCreateRequest): Promise<AiGatewayScope> {
    return normalizeScope(await this.client.iamScopes.create(request));
  }

  async bindScope(name: string, workspaceRef: string): Promise<AiGatewayScope> {
    // SCM binds by slug. Accept a UUID or display name too, resolved on the
    // admin plane because a scope being bound is, by definition, not yet
    // granted to this service account.
    const slug = await this.resolveWorkspaceRef(workspaceRef, ['admin'], true);
    return normalizeScope(await this.client.iamScopes.bindWorkspace(name, slug));
  }

  async deleteScope(name: string): Promise<void> {
    await this.client.iamScopes.delete(name);
  }

  async updateWorkspace(
    workspaceRef: string,
    request: AiGatewayWorkspaceUpdateRequest,
  ): Promise<AiGatewayWorkspaceDetail> {
    const ref = await this.resolveWorkspaceRef(workspaceRef, ['admin']);
    const body: GatewayWorkspaceUpdateRequest = {};
    if (request.name !== undefined) body.name = request.name;
    if (request.description !== undefined) body.description = request.description;
    if (request.icon !== undefined) body.icon = request.icon;
    if (request.defaults !== undefined) body.defaults = request.defaults;
    if (request.usageLimits !== undefined) body.usage_limits = request.usageLimits;
    if (request.rateLimits !== undefined) body.rate_limits = request.rateLimits;

    await this.client.workspaces.update(ref, body);
    // update returns a literal `{}` — the write lands; re-read to display anything.
    return this.getWorkspace(ref, { plane: 'admin' });
  }

  async deleteWorkspace(workspaceRef: string): Promise<void> {
    const ref = await this.resolveWorkspaceRef(workspaceRef, ['admin']);
    // Soft delete. Deliberately no verify-by-get: an archived workspace
    // answers 404 AB08 on both planes even though list --status archived
    // still shows it.
    await this.client.workspaces.delete(ref);
  }

  /**
   * The API accepts only a UUID or slug as a workspace ref — a display name
   * gets a misleading 400 AB01 ("No update fields provided") on writes.
   * Match a user-supplied ref against the workspace list so name | slug |
   * uuid all work. Unmatched refs pass through so the API's own error stands.
   */
  private async resolveWorkspaceRef(
    ref: string,
    planes: AiGatewayPlane[],
    requireSlug = false,
  ): Promise<string> {
    for (const plane of planes) {
      let rows: AiGatewayWorkspace[];
      try {
        rows = await this.listWorkspaces({ plane });
      } catch {
        continue; // e.g. missing grant on this plane — try the next one
      }
      const byRef = rows.find((w) => w.id === ref || w.slug === ref);
      if (byRef) return requireSlug ? byRef.slug : ref;
      const byName = rows.filter((w) => w.name === ref);
      if (byName.length > 1) {
        throw new Error(
          `workspace name '${ref}' is ambiguous (${byName.map((w) => w.slug).join(', ')}) — use a slug or UUID`,
        );
      }
      if (byName.length === 1) return byName[0].slug;
    }
    return ref;
  }

  async getTelemetryCost(opts: AiGatewayCostOptions): Promise<AiGatewayCostReport> {
    const { workspaceSlug: workspaceRef, days = 7, ...rawFilters } = opts;
    const filters = AIGatewayChartFiltersSchema.parse(rawFilters);
    if (!Number.isSafeInteger(days) || days <= 0)
      throw new Error('Expected days to be a positive integer');
    const workspaceSlug = await this.resolveWorkspaceRef(workspaceRef, ['data', 'admin'], true);
    const raw = (await this.client.telemetry.cost({
      workspaceSlug,
      days,
      ...filters,
    })) as {
      data: {
        isQuotaExceeded: boolean;
        records: Array<{ x: string; y: number }>;
        total: number;
        avg: number;
      };
    };
    // Every cost value is CENTS — the SDK never converts; conversion is a
    // display concern (renderer divides by 100).
    return {
      workspaceSlug,
      days,
      totalCents: raw.data.total,
      totalUsd: raw.data.total / 100,
      avgCents: raw.data.avg,
      avgUsd: raw.data.avg / 100,
      quotaExceeded: raw.data.isQuotaExceeded,
      records: raw.data.records.map((r) => ({
        date: r.x,
        costCents: r.y,
        costUsd: r.y / 100,
      })),
    };
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
